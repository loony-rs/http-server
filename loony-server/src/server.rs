use crate::{connection::Connection, error::*, response::HttpResponse};
use ahash::AHashMap;
use async_std::task::block_on;
use loony_service::{IntoServiceFactory, Service, ServiceFactory};
use crate::{app_service::AppHttpService, extensions::Extensions, request::HttpRequest, resource::ResourceService, service::ServiceRequest};
use std::{cell::RefCell, marker::PhantomData, net::TcpStream, rc::Rc, time::Duration};
use socket2::{Socket, Domain, Type};
use std::net::TcpListener;

pub struct Run {
    routes: AHashMap<String, Rc<RefCell<ResourceService>>>,
    extensions: Rc<Extensions>,
    listener: std::net::TcpListener,
}

impl Run {
    fn run(&self) {
        loop {
            let (stream, _) = self.listener.accept().unwrap();            
            self.handle_connection(stream).unwrap();
        }
    }

    /// Handles an individual TCP connection
    fn handle_connection(
        &self, 
        stream: TcpStream,
    ) -> Result<(), ServerError> {
        let mut connection = Connection::new(stream)?;
        let bytes_read = connection.read_http_response()?;
        let request = self.request(&bytes_read)?;
        let response = self.response(request)?;
        connection.write_str(&response)?;
        connection.close()?;
        Ok(())
    }


    /// Parses raw HTTP request data into a structured Request object
    fn request(&self, buffer: &[u8]) -> Result<HttpRequest, ServerError> {
        let mut request = HttpRequest::new();
        let _ = request.parse(buffer).unwrap();
        Ok(request.into())
    }

    /// Handles an HTTP request and generates an appropriate response
    fn response(
        &self,
        request: HttpRequest,
    ) -> Result<String, ServerError> {
        let path = request.uri.as_ref()
            .ok_or(HandlerError::MissingUri)?;
        
        if let Some(service) = self.routes.get(path) {
            self.execute_service(service.clone(), request)
        } else {
            Ok(HttpResponse::bad_request().build())
        }
    }

     /// Executes the appropriate service for the request
    fn execute_service(
        &self,
        service: Rc<RefCell<ResourceService>>,
        request: HttpRequest
    ) -> Result<String, ServerError> {
        let service_request = ServiceRequest {
            req: request,
            extensions: self.extensions.clone(),
        };

        let mut service_clone = Rc::clone(&service);
        let future = service_clone.call(service_request);
        
        match block_on(future) {
            Ok(response) => {
                Ok(response.0)
            }
            Err(_) => {
                Ok(HttpResponse::internal_server_error().build())
            }
        }
    }

}
pub struct ServeHttpService<F, I, T> 
where F: Fn() -> I + Send + Clone + 'static,
I: IntoServiceFactory<T>,
T: ServiceFactory 
{
    app: F,
    _p: PhantomData<T>
}

impl<F, I, T> ServeHttpService<F, I, T> 
where F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<T>,
    T: ServiceFactory<Request=(), Config = (), Service = AppHttpService>,
{
    pub fn new(app: F) -> Self {
        ServeHttpService { app, _p: PhantomData }
    }
    
    pub fn run(&mut self, listener: std::net::TcpListener) {
        let (routes, extensions) = self.new_service().unwrap();
       let y = Run {
            routes,
            extensions: Rc::new(extensions),
            listener,
        };
        y.run();
    }

    // /// Starts the server and initializes all services
    fn new_service(&mut self) ->  Result<(AHashMap<String, Rc<RefCell<ResourceService>>>, Extensions), ServerError>
    {
        let app = (self.app)();
        let app_factory = app.into_factory();
        let app_service = app_factory.new_service(());
        
        let http_service: Result<AppHttpService, T::InitError> = block_on(app_service);
        
        match http_service {
            Ok(service) => {
                Ok((service.routes, service.extensions))
            }
            Err(_) => {
                Err(ServerError::service_init_error(String::from("Failed to initialize app services.")))
            }
        }
    }


}

pub struct HttpServer<F, I, T> 
where F: Fn() -> I + Send + Clone + 'static,
I: IntoServiceFactory<T>,
T: ServiceFactory,
{
    app: F,
    config: ServerConfig,
    port: i32,
    _p: PhantomData<T>
}

impl<F, I, T> HttpServer<F, I, T> 
where F: Fn() -> I + Send + Clone + 'static,
    I: IntoServiceFactory<T>,
    T: ServiceFactory<Request=(), Config = (), Service = AppHttpService>,
{
    pub fn new(app: F) -> Self {
        Self { 
            app,
            config: ServerConfig::default(),
            port: 2443,
            _p: PhantomData,
        }
    }

    /// Configures the server with custom settings
    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn bind(mut self, port: i32) -> Self {
        self.port = port;
        self
    }
    /// Runs the HTTP server and starts accepting connections
    ///
    /// This method blocks the current thread and runs the server indefinitely
    ///
    /// # Panics
    ///
    /// Panics if the server fails to start or service initialization fails
    pub async fn run(&mut self) {
        let mut servers = Vec::new();
        for _ in 0..4 {
            let x = self.app.clone();
            let _ = self.config.clone();
            let socket = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
            socket.set_reuse_port(true).unwrap();
            socket.bind(&format!("127.0.0.1:{}", self.port).parse::<std::net::SocketAddr>().unwrap().into()).unwrap();
            socket.listen(128).unwrap();
            let listener: TcpListener = socket.into();

            let handle = tokio::spawn(async move {
                let mut t =ServeHttpService::new(x);
                t.run(listener);
            });
            servers.push(handle);
        }
            
        // Run all servers
        futures_util::future::join_all(servers).await;
    }

}

/// Server configuration
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
