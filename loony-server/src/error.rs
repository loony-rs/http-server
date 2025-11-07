/// Comprehensive error types for the HTTP server
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Service initialization failed: {message}")]
    ServiceInitializationFailed {
        // source: Box<dyn std::error::Error + Send + Sync>,
        message: String,
    },
    #[error("Failed to build server listener: {source}")]
    ListenerBuildError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("I/O error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
    #[error("Connection error: {source}")]
    ConnectionError {
        #[from]
        source: ConnectionError,
    },
    #[error("Parse error: {source}")]
    ParseError {
        #[from]
        source: ParseError,
    },
    #[error("Handler error: {source}")]
    HandlerError {
        #[from]
        source: HandlerError,
    },
    #[error("Service execution error: {source}")]
    ServiceError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Configuration error: {message}")]
    ConfigError {
        message: String,
    },
    #[error("Timeout error: {operation} took too long")]
    TimeoutError {
        operation: String,
    },
    #[error("Resource not found: {resource}")]
    ResourceNotFound {
        resource: String,
    },
}

/// Connection-level errors
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Failed to read from connection: {source}")]
    ReadError {
        // #[from]
        source: std::io::Error,
    },
    #[error("Failed to write to connection: {source}")]
    WriteError {
        // #[from]
        source: std::io::Error,
    },
    #[error("Connection closed unexpectedly")]
    ConnectionClosed,
    #[error("Connection timeout")]
    Timeout,
    #[error("Protocol error: {message}")]
    ProtocolError {
        message: String,
    },
}


/// Request parsing errors
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid HTTP method: {method}")]
    InvalidMethod {
        method: String,
    },
    #[error("Invalid URI: {uri}")]
    InvalidUri {
        uri: String,
    },
    #[error("Invalid HTTP version: {version}")]
    InvalidVersion {
        version: String,
    },
    #[error("Malformed headers: {reason}")]
    MalformedHeaders {
        reason: String,
    },
    #[error("Buffer overflow: tried to read {attempted} bytes into {capacity} byte buffer")]
    BufferOverflow {
        attempted: usize,
        capacity: usize,
    },
    #[error("Invalid UTF-8 sequence in request")]
    InvalidUtf8,
    #[error("Content length mismatch: expected {expected}, got {actual}")]
    ContentLengthMismatch {
        expected: usize,
        actual: usize,
    },
}

/// Request handling errors
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("Missing URI in request")]
    MissingUri,
    #[error("Route not found: {route}")]
    RouteNotFound {
        route: String,
    },
    #[error("Method not allowed for route: {route}")]
    MethodNotAllowed {
        route: String,
    },
    #[error("Service unavailable: {reason}")]
    ServiceUnavailable {
        reason: String,
    },
    #[error("Payload too large: {size} bytes")]
    PayloadTooLarge {
        size: usize,
    },
    #[error("Unsupported media type: {content_type}")]
    UnsupportedMediaType {
        content_type: String,
    },
    #[error("Internal server error: {source}")]
    InternalError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Service factory errors
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Service creation failed: {reason}")]
    CreationFailed {
        reason: String,
    },
    #[error("Service initialization timeout")]
    InitializationTimeout,
    #[error("Service dependency error: {service}")]
    DependencyError {
        service: String,
    },
}

// Convenient type aliases
pub type ServerResult<T> = Result<T, ServerError>;
pub type ConnectionResult<T> = Result<T, ConnectionError>;
pub type ParseResult<T> = Result<T, ParseError>;
pub type HandlerResult<T> = Result<T, HandlerError>;

// Implementation for converting various error types
impl From<Box<dyn std::error::Error + Send + Sync>> for ServerError {
    fn from(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        ServerError::ServiceError { source }
    }
}

impl From<String> for ServerError {
    fn from(message: String) -> Self {
        ServerError::ConfigError { message }
    }
}

impl From<&str> for ServerError {
    fn from(message: &str) -> Self {
        ServerError::ConfigError {
            message: message.to_string(),
        }
    }
}

// Helper methods for common error cases
impl ServerError {
    // pub fn service_init_error<E: std::error::Error + Send + Sync + 'static>(source: E) -> Self {
    //     ServerError::ServiceInitializationFailed {
    //         message: Box::new(source),
    //     }
    // }
    pub fn service_init_error(msg: String) -> Self {
        ServerError::ServiceInitializationFailed {
            message: msg,
        }
    }

    pub fn timeout(operation: &str) -> Self {
        ServerError::TimeoutError {
            operation: operation.to_string(),
        }
    }

    pub fn not_found(resource: &str) -> Self {
        ServerError::ResourceNotFound {
            resource: resource.to_string(),
        }
    }
}

impl HandlerError {
    pub fn internal_error<E: std::error::Error + Send + Sync + 'static>(source: E) -> Self {
        HandlerError::InternalError {
            source: Box::new(source),
        }
    }

    pub fn route_not_found(route: &str) -> Self {
        HandlerError::RouteNotFound {
            route: route.to_string(),
        }
    }
}

impl ParseError {
    pub fn buffer_overflow(attempted: usize, capacity: usize) -> Self {
        ParseError::BufferOverflow {
            attempted,
            capacity,
        }
    }

    pub fn malformed_headers(reason: &str) -> Self {
        ParseError::MalformedHeaders {
            reason: reason.to_string(),
        }
    }
}