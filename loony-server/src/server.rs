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
use tokio::runtime::Builder as RuntimeBuilder;

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

    /// Read the request, dispatch it, write the response.
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

        if let Some((service, params)) = self.route.find_route(&path) {
            self.call_service(service, params, request).await
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
        params: Vec<String>,
        request: HttpRequest,
    ) -> Result<String, ServerError> {
        let service_request = ServiceRequest {
            req: request,
            extensions: self.extensions.clone(),
            path_params: Rc::new(params),
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
