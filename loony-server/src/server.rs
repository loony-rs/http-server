use crate::{builder::Builder, connection::Connection, error::*, response::HttpResponse};
use ahash::AHashMap;
use async_std::task::block_on;
use loony_service::{IntoServiceFactory, Service, ServiceFactory};
use crate::{app_service::AppHttpService, extensions::Extensions, request::HttpRequest, resource::ResourceService, service::ServiceRequest};
use std::{cell::RefCell, marker::PhantomData, net::TcpStream, rc::Rc, time::Duration};

// pub type AppInstance = Box<dyn Fn() -> App + 'static>;

pub struct HttpServer<F, I, T> 
where F: Fn() -> I + Send + Clone + 'static,
I: IntoServiceFactory<T>,
T: ServiceFactory,
{
    app: F,
    builder: Builder,
    routes: AHashMap<String, Rc<RefCell<ResourceService>>>,
    extensions: Rc<Extensions>,
    config: ServerConfig,
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
            builder: Builder::new(),
            routes: AHashMap::new(),
            extensions: Rc::new(Extensions::new()),
            config: ServerConfig::default(),
            _p: PhantomData,
        }
    }

    /// Configures the server with custom settings
    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }

    /// Starts the server and initializes all services
    fn initialize_services(&mut self) ->  ServerResult<()> {
        let app = (self.app)();
        let app_factory = app.into_factory();
        let app_service = app_factory.new_service(());
        
        let http_service: Result<AppHttpService, T::InitError> = block_on(app_service);
        
        match http_service {
            Ok(service) => {
                self.routes = service.routes;
                self.extensions = Rc::new(service.extensions);
                Ok(())
            }
            Err(_) => {
                Err(ServerError::service_init_error(String::from("Failed to initialize app services.")))
            }
        }
    }

    /// Runs the HTTP server and starts accepting connections
    ///
    /// This method blocks the current thread and runs the server indefinitely
    ///
    /// # Panics
    ///
    /// Panics if the server fails to start or service initialization fails
    pub fn run(&mut self) {
        
        if let Err(e) = self.initialize_services() {
            panic!("Failed to start server: {}", e);
        }

        let listener = self.builder.build();
            // .expect("Failed to build server listener");

        loop {
            let stream = listener.recv().unwrap();
            // let response_builder = Response::new(&self.routes, self.extensions.clone());            
            self.handle_connection(stream).unwrap();
        }
    }

  /// Handles an individual TCP connection
    fn handle_connection(
        &self, 
        stream: TcpStream,
    ) -> Result<(), ServerError> {
        let mut connection = Connection::new(stream);
        let mut buffer = [0; 1024];
        
        // Read request
        let bytes_read = connection.read(&mut buffer)?;
        if bytes_read == 0 {
            return Ok(()); // Empty request, skip processing
        }

        // Parse request
        let request = self.parse_request(&buffer[..bytes_read])?;
        
        // Handle request and send response
        let response = self.handle_request(request)?;
        connection.write(&response);
        
        connection.close();
        Ok(())
    }


    /// Parses raw HTTP request data into a structured Request object
    fn parse_request(&self, buffer: &[u8]) -> Result<HttpRequest, ServerError> {
        // let mut headers = [EMPTY_HEADER; 16];
        let mut request = HttpRequest::new();
        let _ = request.parse(buffer).unwrap();
        Ok(request.into())
    }

    /// Handles an HTTP request and generates an appropriate response
    fn handle_request(
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
