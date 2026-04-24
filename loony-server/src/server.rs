use crate::{
    app_service::AppHttpService, extensions::Extensions, request::HttpRequest,
    resource::FinalRouteService, service::ServiceRequest,
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
    marker::PhantomData,
    net::{TcpListener, TcpStream},
    rc::Rc,
    time::Duration,
};
use tokio::net::TcpListener as TokioListener;

// ---------------------------------------------------------------------------
// Run — owns the live router and accepts connections
// ---------------------------------------------------------------------------

struct Run {
    extensions: Rc<Extensions>,
    route: AllRouteServices,
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
                    // Convert to std TcpStream for the synchronous Connection
                    // type. Step 5 replaces this with a fully async tokio I/O
                    // path so the conversion goes away.
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

    /// Read the request, dispatch it, write the response.
    ///
    /// The sync I/O calls (`Connection::new`, `read_http_response`,
    /// `write_str`, `close`) block the thread. This is the known limitation
    /// called out in the Step 1 tradeoffs and is fixed in Step 5.
    async fn handle_connection(&self, stream: TcpStream) -> Result<(), ServerError> {
        let mut connection = Connection::new(stream)?;
        let bytes_read = connection.read_http_response()?;
        let request = self.parse_request(&bytes_read)?;
        let response = self.dispatch(request).await?;
        connection.write_str(&response)?;
        connection.close()?;
        Ok(())
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

        if let Some(service) = self.route.find_route(&path) {
            self.call_service(service, request).await
        } else {
            Ok(HttpResponse::bad_request().build())
        }
    }

    /// Call the matched service and await its future on the tokio runtime.
    ///
    /// `service.borrow_mut().call(...)` creates the future and immediately
    /// drops the `RefMut` borrow (at the statement semicolon). The returned
    /// `Pin<Box<dyn Future>>` does not borrow from the `RefCell`, so it is
    /// safe to `.await` after the borrow is released.
    async fn call_service(
        &self,
        service: Rc<RefCell<FinalRouteService>>,
        request: HttpRequest,
    ) -> Result<String, ServerError> {
        let service_request = ServiceRequest {
            req: request,
            extensions: self.extensions.clone(),
        };

        // The RefMut is dropped at the semicolon; the future outlives the borrow.
        let future = service.borrow_mut().call(service_request);

        match future.await {
            Ok(response) => Ok(response.0),
            Err(_) => Ok(HttpResponse::internal_server_error().build()),
        }
    }
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
        let (extensions, route) = match self.new_service().await {
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
            listener: tokio_listener,
        }
        .run()
        .await;
    }

    /// Initialise the app factory.
    ///
    /// Uses `.await` — the future is already `Ready` (see `AppFactory`), so
    /// this resolves immediately. The route `register()` calls that happen
    /// inside `AppFactory::new_service` use `futures::executor::block_on`
    /// (see route.rs / resource.rs), which is safe for pure-computation
    /// futures that do not touch tokio primitives.
    async fn new_service(&mut self) -> Result<(Extensions, AllRouteServices), ServerError> {
        let app = (self.app)();
        let app_factory = app.into_factory();
        let app_service_future = app_factory.new_service(());

        match app_service_future.await {
            Ok(service) => Ok((service.extensions, service.route)),
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

    /// Bind the socket and start accepting connections.
    ///
    /// Returns `Err` if the port is invalid or the socket cannot be bound
    /// (e.g. the port is already in use).
    ///
    /// Uses a `LocalSet` so that the worker task (which holds `Rc<...>`
    /// types that are `!Send`) can be scheduled on the current thread
    /// without requiring `Arc` or `Send` bounds.
    ///
    /// Step 5 replaces the `0..1` loop with a configurable worker count,
    /// each pinned to its own OS thread via `tokio::task::spawn_blocking` +
    /// a dedicated `LocalSet`.
    pub async fn run(self) -> Result<(), ServerError> {
        let port = u16::try_from(self.port).map_err(|_| ServerError::ConfigError {
            message: format!("invalid port {}: must be 0–65535", self.port),
        })?;

        let local = tokio::task::LocalSet::new();

        for _ in 0..1 {
            let app = self.app.clone();

            let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
            socket.set_reuse_port(true)?;
            let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
            socket.bind(&addr.into())?;
            socket.listen(128)?;
            let listener: TcpListener = socket.into();

            local.spawn_local(async move {
                ServeHttpService::new(app).run(listener).await;
            });
        }

        local.await;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub max_connections: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 3005,
            max_connections: 1000,
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
        }
    }
}
