use crate::{
    app_service::AppHttpService,
    extensions::Extensions,
    middleware::{BoxFuture, Middleware, Next},
    request::HttpRequest,
    resource::FinalRouteService,
    service::{ServiceRequest, ServiceResponse},
};
use crate::{
    connection::Connection,
    error::{HandlerError, ParseError, ServerError},
    response::HttpResponse,
    router::AllRouteServices,
};

use loony_service::{IntoServiceFactory, Service, ServiceFactory};
use socket2::{Domain, Socket, Type};
use std::{
    cell::RefCell,
    io,
    marker::PhantomData,
    net::{TcpListener, TcpStream},
    rc::Rc,
    time::Duration,
};
use tokio::net::TcpListener as TokioListener;
use tokio::runtime::Builder as RuntimeBuilder;

// ---------------------------------------------------------------------------
// Run — owns the live router and accepts connections
// ---------------------------------------------------------------------------

struct Run {
    extensions: Rc<Extensions>,
    route: AllRouteServices,
    middlewares: Vec<Rc<dyn Middleware>>,
    listener: TokioListener,
}

impl Run {
    /// Accept loop — runs forever on the current task.
    ///
    /// `accept().await` yields control to the tokio scheduler between
    /// connections, so other local tasks can make progress.
    async fn run(self) {
        loop {
            match self.listener.accept().await {
                Ok((tokio_stream, _addr)) => {
                    // Convert to std TcpStream for the synchronous Connection type.
                    match tokio_stream.into_std() {
                        Ok(std_stream) => {
                            if let Err(e) = self.handle_connection(std_stream).await {
                                eprintln!("connection error: {e}");
                            }
                        },
                        Err(e) => eprintln!("stream conversion error: {e}"),
                    }
                },
                Err(e) => eprintln!("accept error: {e}"),
            }
        }
    }

    /// Serve all requests on a single TCP connection, reusing it while the
    /// client keeps the connection alive.
    async fn handle_connection(&self, stream: TcpStream) -> Result<(), ServerError> {
        let mut connection = Connection::new(stream)?;
        loop {
            let bytes = match connection.read_http_response() {
                Ok(b) => b,
                // Client closed or went quiet — end the loop cleanly.
                Err(e) if is_disconnect(&e) => break,
                Err(e) => return Err(e.into()),
            };

            let request = self.parse_request(&bytes)?;
            let keep_alive = is_keep_alive(&request);
            let response = self.dispatch(request).await?;
            let response = inject_connection_header(response, keep_alive);
            connection.write_str(&response)?;

            if !keep_alive {
                break;
            }
        }
        connection.close().map_err(Into::into)
    }

    /// Parse raw bytes into a structured `HttpRequest`.
    fn parse_request(&self, buffer: &[u8]) -> Result<HttpRequest, ServerError> {
        let mut request = HttpRequest::new();
        request
            .parse(buffer)
            .map_err(|reason| ParseError::MalformedHeaders {
                reason: reason.to_string(),
            })?;
        Ok(request.into())
    }

    /// Route the request and produce a serialised HTTP response string.
    async fn dispatch(&self, request: HttpRequest) -> Result<String, ServerError> {
        let path = request
            .uri
            .as_ref()
            .ok_or(HandlerError::MissingUri)?
            .clone();

        if let Some((service, params)) = self.route.find_route(&path) {
            self.call_service(service, params, request).await
        } else {
            Ok(HttpResponse::bad_request().build())
        }
    }

    async fn call_service(
        &self,
        service: Rc<RefCell<FinalRouteService>>,
        params: Vec<String>,
        request: HttpRequest,
    ) -> Result<String, ServerError> {
        let service_request = ServiceRequest {
            req: request,
            extensions: self.extensions.clone(),
            path_params: Rc::new(params),
        };

        let response = if self.middlewares.is_empty() {
            // Fast path — no middleware overhead.
            let future = service.borrow_mut().call(service_request);
            match future.await {
                Ok(r) => r,
                Err(_) => ServiceResponse(HttpResponse::internal_server_error().build()),
            }
        } else {
            let chain = build_middleware_chain(&self.middlewares, service);
            chain(service_request).await
        };

        Ok(response.0)
    }
}

// ---------------------------------------------------------------------------
// Middleware chain builder
// ---------------------------------------------------------------------------

/// Build the execution chain from a slice of registered middlewares and a
/// terminal `FinalRouteService`.
///
/// Middlewares are stored in registration order `[M0, M1, ...]`.  We fold
/// over them in reverse so that `M0` ends up as the outermost wrapper —
/// meaning `M0` runs first on the request path and last on the response path.
fn build_middleware_chain(
    middlewares: &[Rc<dyn Middleware>],
    service: Rc<RefCell<FinalRouteService>>,
) -> Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> {
    // Innermost layer: call the actual route handler.
    let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> =
        Rc::new(move |req: ServiceRequest| {
            let svc = Rc::clone(&service);
            Box::pin(async move {
                let fut = svc.borrow_mut().call(req);
                match fut.await {
                    Ok(r) => r,
                    Err(_) => ServiceResponse(HttpResponse::internal_server_error().build()),
                }
            }) as BoxFuture<ServiceResponse>
        });

    middlewares.iter().rev().fold(base, |inner, mw| {
        let mw = Rc::clone(mw);
        Rc::new(move |req: ServiceRequest| {
            let next = Next::new(Rc::clone(&inner));
            mw.handle(req, next)
        }) as Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>>
    })
}

// ---------------------------------------------------------------------------
// Keep-alive helpers
// ---------------------------------------------------------------------------

/// True for errors that mean the client side of the connection closed or timed
/// out.  These end the keep-alive loop without logging an error.
fn is_disconnect(e: &io::Error) -> bool {
    use io::ErrorKind::*;
    matches!(
        e.kind(),
        UnexpectedEof | ConnectionReset | ConnectionAborted | BrokenPipe | TimedOut | WouldBlock
    )
}

/// Decide whether to keep the connection alive for this request.
///
/// HTTP/1.1 defaults to keep-alive; the client must send `Connection: close`
/// to opt out.  HTTP/1.0 defaults to close; the client must send
/// `Connection: keep-alive` to opt in.
fn is_keep_alive(req: &HttpRequest) -> bool {
    let wants_close = req
        .headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("connection") && v.trim().eq_ignore_ascii_case("close"));
    let wants_keep = req.headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("connection") && v.trim().eq_ignore_ascii_case("keep-alive")
    });

    match req.version {
        Some(1) => !wants_close,  // HTTP/1.1 — on by default
        Some(0) => wants_keep,    // HTTP/1.0 — off by default
        _ => false,
    }
}

/// Inject a `Connection: keep-alive` or `Connection: close` header into an
/// already-serialised HTTP response, unless the handler set it explicitly.
///
/// Insertion point: after the last header's `\r\n`, immediately before the
/// blank line that separates headers from the body.
fn inject_connection_header(mut response: String, keep_alive: bool) -> String {
    let header_end = match response.find("\r\n\r\n") {
        Some(p) => p,
        None => return response,
    };

    // Skip if the handler already emitted a Connection header.
    let already_set = response[..header_end]
        .split("\r\n")
        .skip(1) // skip status line
        .any(|line| line.to_ascii_lowercase().starts_with("connection:"));

    if already_set {
        return response;
    }

    let value = if keep_alive { "keep-alive" } else { "close" };
    // Insert after the last header's \r\n (header_end + 2), before the blank \r\n.
    response.insert_str(header_end + 2, &format!("Connection: {value}\r\n"));
    response
}

// ---------------------------------------------------------------------------
// ServeHttpService — initialises the app and hands off to Run
// ---------------------------------------------------------------------------

struct ServeHttpService<F, I, T>
where
    F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<T>,
    T: ServiceFactory,
{
    app: F,
    _p: PhantomData<T>,
}

impl<F, I, T> ServeHttpService<F, I, T>
where
    F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<T>,
    T: ServiceFactory<Request = (), Config = (), Service = AppHttpService>,
{
    fn new(app: F) -> Self {
        ServeHttpService {
            app,
            _p: PhantomData,
        }
    }

    /// Build services, then hand the live router to `Run`.
    pub async fn run(mut self, std_listener: TcpListener) {
        let (extensions, route, middlewares) = match self.new_service().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("service init failed: {e}");
                return;
            },
        };

        let tokio_listener = match TokioListener::from_std(std_listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("listener setup failed: {e}");
                return;
            },
        };

        Run {
            extensions: Rc::new(extensions),
            route,
            middlewares,
            listener: tokio_listener,
        }
        .run()
        .await;
    }

    async fn new_service(
        &mut self,
    ) -> Result<(Extensions, AllRouteServices, Vec<Rc<dyn Middleware>>), ServerError> {
        let app = (self.app)();
        let app_factory = app.into_factory();
        let app_service_future = app_factory.new_service(());

        match app_service_future.await {
            Ok(service) => Ok((service.extensions, service.route, service.middlewares)),
            Err(_) => Err(ServerError::service_init_error(
                "failed to initialise app services".to_string(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// HttpServer — public API
// ---------------------------------------------------------------------------

pub struct HttpServer<F, I, T>
where
    F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<T>,
    T: ServiceFactory,
{
    app: F,
    config: ServerConfig,
    port: i32,
    _p: PhantomData<T>,
}

impl<F, I, T> HttpServer<F, I, T>
where
    F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<T>,
    T: ServiceFactory<Request = (), Config = (), Service = AppHttpService>,
{
    pub fn new(app: F) -> Self {
        Self {
            app,
            config: ServerConfig::default(),
            port: 2443,
            _p: PhantomData,
        }
    }

    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn bind(mut self, port: i32) -> Self {
        self.port = port;
        self
    }

    pub fn workers(mut self, count: usize) -> Self {
        self.config.workers = count;
        self
    }

    /// Bind the socket and start accepting connections.
    ///
    /// Each worker gets its own OS thread running a `current_thread` Tokio
    /// runtime with a `LocalSet`.  This lets `Rc<...>` types (`AllRouteServices`,
    /// `Extensions`) stay `!Send` without requiring `Arc`.  `SO_REUSEPORT`
    /// lets all workers bind the same port; the kernel distributes incoming
    /// connections across them.
    pub async fn run(self) -> Result<(), ServerError> {
        let port = u16::try_from(self.port).map_err(|_| ServerError::ConfigError {
            message: format!("invalid port {}: must be 0–65535", self.port),
        })?;

        let workers = self.config.workers;

        for i in 0..workers {
            let app = self.app.clone();

            let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
            socket.set_reuse_port(true)?;
            let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
            socket.bind(&addr.into())?;
            socket.listen(128)?;
            let listener: TcpListener = socket.into();

            // Each worker owns its own single-thread runtime + LocalSet so
            // !Send types (Rc, RefCell) never cross thread boundaries.
            std::thread::Builder::new()
                .name(format!("loony-worker-{i}"))
                .spawn(move || {
                    let rt = RuntimeBuilder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to build worker runtime");
                    let local = tokio::task::LocalSet::new();
                    local.spawn_local(async move {
                        ServeHttpService::new(app).run(listener).await;
                    });
                    rt.block_on(local);
                })
                .map_err(|e| ServerError::ConfigError {
                    message: format!("failed to spawn worker thread: {e}"),
                })?;
        }

        // Workers loop forever on their own threads.
        // Block until the process is signalled (e.g. Ctrl-C).
        std::future::pending::<()>().await;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub workers: usize,
    pub max_connections: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Self {
            port: 3005,
            workers,
            max_connections: 1000,
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::HttpRequest;

    fn make_request(version: u8, connection_header: Option<&str>) -> HttpRequest {
        let mut req = HttpRequest::new();
        req.version = Some(version);
        if let Some(val) = connection_header {
            req.headers.push(("Connection".to_string(), val.to_string()));
        }
        req
    }

    // --- is_keep_alive ---

    #[test]
    fn http11_keep_alive_by_default() {
        assert!(is_keep_alive(&make_request(1, None)));
    }

    #[test]
    fn http11_close_on_connection_close() {
        assert!(!is_keep_alive(&make_request(1, Some("close"))));
    }

    #[test]
    fn http10_close_by_default() {
        assert!(!is_keep_alive(&make_request(0, None)));
    }

    #[test]
    fn http10_keep_alive_on_opt_in() {
        assert!(is_keep_alive(&make_request(0, Some("keep-alive"))));
    }

    // --- inject_connection_header ---

    #[test]
    fn inject_adds_keep_alive() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello".to_string();
        let out = inject_connection_header(resp, true);
        assert!(out.contains("Connection: keep-alive\r\n"));
        assert!(out.contains("\r\n\r\nhello"));
    }

    #[test]
    fn inject_adds_close() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello".to_string();
        let out = inject_connection_header(resp, false);
        assert!(out.contains("Connection: close\r\n"));
    }

    #[test]
    fn inject_skips_if_already_present() {
        let resp =
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 5\r\n\r\nhello".to_string();
        let out = inject_connection_header(resp.clone(), true);
        // Must not add a second Connection header.
        assert_eq!(out.matches("Connection:").count(), 1);
    }

    #[test]
    fn inject_no_body() {
        let resp = "HTTP/1.1 204 No Content\r\n\r\n".to_string();
        let out = inject_connection_header(resp, true);
        assert!(out.contains("Connection: keep-alive\r\n"));
        assert!(out.ends_with("\r\n\r\n"));
    }
}
