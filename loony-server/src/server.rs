use crate::{builder::Builder, connection::Connection, error::*};
use ahash::AHashMap;
use async_std::task::block_on;
use loony_service::{IntoServiceFactory, Service, ServiceFactory};
use crate::{App, app_service::AppHttpService, extensions::Extensions, request::{EMPTY_HEADER, HttpRequest, Request}, resource::ResourceService, service::ServiceRequest};
use std::{cell::RefCell, marker::PhantomData, net::TcpStream, rc::Rc, time::Duration};

static RESPONSE_OK: &str = "HTTP/1.1 200 OK\r\n\r\n";
static RESPONSE_NOT_FOUND: &str = "HTTP/1.1 401 NOT FOUND\r\n\r\nNOT FOUND";
static RESPONSE_INTERNAL_ERROR: &str = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\nNOT FOUND";

pub type AppInstance = Box<dyn Fn() -> App + 'static>;
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
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration
    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }

    /// Starts the server and initializes all services
    ///
    /// # Errors
    ///
    /// Returns an error if service initialization fails
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
            Err(e) => {
                // eprintln!("Failed to initialize HTTP service: {:?}", e);
                Err(ServerError::service_init_error(String::from("")))
                // Err(String::from(""))
            }
        }
    }

    fn start(&mut self) {
        let app = (self.app)();
        let app_factory = app.into_factory();
        let app_service = app_factory.new_service(());
        let http_service:Result<AppHttpService, <T as ServiceFactory>::InitError> = block_on(app_service);
        if let Ok(http_service) = http_service {
            let exts = http_service.extensions;
            self.routes = http_service.routes;
            self.extensions = Rc::new(exts);
        };
    }

    // pub fn run(&mut self) {
    //     self.start();
    //     let a = self.builder.run();
    //     println!("Http Server is running on Port: 3005");
    //     self.accept(a);
    // }

    /// Runs the HTTP server and starts accepting connections
    ///
    /// This method blocks the current thread and runs the server indefinitely
    ///
    /// # Panics
    ///
    /// Panics if the server fails to start or service initialization fails
    pub fn run(&mut self) {
        log::info!("Starting HTTP server...");
        
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
    fn parse_request(&self, buffer: &[u8]) -> Result<Request, ServerError> {
        let mut headers = [EMPTY_HEADER; 16];
        let mut request = Request::new(&mut headers);
        request.parse(buffer);
        Ok(request.into())
    }

    /// Handles an HTTP request and generates an appropriate response
    fn handle_request(
        &self,
        request: Request,
    ) -> Result<String, ServerError> {
        let path = request.uri.as_ref()
            .ok_or(HandlerError::MissingUri)?;

        let (path, query_params) = self.parse_uri(path);
        
        if let Some(service) = self.routes.get(&path) {
            self.execute_service(service.clone(), request, query_params)
        } else {
            Ok(RESPONSE_NOT_FOUND.to_string())
        }
    }

    /// Parses URI into path and query parameters
    fn parse_uri(&self, uri: &str) -> (String, Vec<String>) {
        let parts: Vec<&str> = uri.split('?').collect();
        let path = parts.first().map(|&p| p.to_string()).unwrap_or_default();
        let query_params = parts.get(1)
            .map(|&q| q.split('&').map(String::from).collect())
            .unwrap_or_default();
        
        (path, query_params)
    }

     /// Executes the appropriate service for the request
    fn execute_service(
        &self,
        service: Rc<RefCell<ResourceService>>,
        request: Request,
        query_params: Vec<String>
    ) -> Result<String, ServerError> {
        let service_request = ServiceRequest(HttpRequest {
            url: String::from(request.uri.unwrap()),
            extensions: self.extensions.clone(),
            params: Some(query_params),
        });

        let mut service_clone = Rc::clone(&service);
        let future = service_clone.call(service_request);
        
        match block_on(future) {
            Ok(response) => {
                let mut http_response = String::from(RESPONSE_OK);
                http_response.push_str(&response.0.value);
                Ok(http_response)
            }
            Err(e) => {
                log::error!("Service execution error: {:?}", e);
                Ok(RESPONSE_INTERNAL_ERROR.to_string())
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
