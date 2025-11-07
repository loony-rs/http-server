// #[derive(Clone)]
// pub struct HttpResponse {
//     pub value: String,
// }


use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub enum HttpVersion {
    Http1_0,
    Http1_1,
    Http2,
    Http3,
}

impl Display for HttpVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpVersion::Http1_0 => write!(f, "HTTP/1.0"),
            HttpVersion::Http1_1 => write!(f, "HTTP/1.1"),
            HttpVersion::Http2 => write!(f, "HTTP/2"),
            HttpVersion::Http3 => write!(f, "HTTP/3"),
        }
    }
}

// Helper trait for easy status code creation
pub trait IntoStatusCode {
    fn into_status_code(self) -> StatusCode;
}

impl IntoStatusCode for StatusCode {
    fn into_status_code(self) -> StatusCode {
        self
    }
}

impl IntoStatusCode for u16 {
    fn into_status_code(self) -> StatusCode {
        StatusCode::from_u16(self).unwrap_or(StatusCode::InternalServerError)
    }
}

impl StatusCode {
    // Common success methods
    pub fn ok() -> Self { StatusCode::Ok }
    pub fn created() -> Self { StatusCode::Created }
    pub fn no_content() -> Self { StatusCode::NoContent }
    
    // Common client error methods
    pub fn bad_request() -> Self { StatusCode::BadRequest }
    pub fn unauthorized() -> Self { StatusCode::Unauthorized }
    pub fn forbidden() -> Self { StatusCode::Forbidden }
    pub fn not_found() -> Self { StatusCode::NotFound }
    pub fn method_not_allowed() -> Self { StatusCode::MethodNotAllowed }
    
    // Common server error methods
    pub fn internal_server_error() -> Self { StatusCode::InternalServerError }
    pub fn not_implemented() -> Self { StatusCode::NotImplemented }
    pub fn bad_gateway() -> Self { StatusCode::BadGateway }
    pub fn service_unavailable() -> Self { StatusCode::ServiceUnavailable }
    
    // Redirection methods
    pub fn moved_permanently() -> Self { StatusCode::MovedPermanently }
    pub fn found() -> Self { StatusCode::Found }
    pub fn temporary_redirect() -> Self { StatusCode::TemporaryRedirect }
    pub fn permanent_redirect() -> Self { StatusCode::PermanentRedirect }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusCode {
    // 1xx Informational
    Continue = 100,
    SwitchingProtocols = 101,
    Processing = 102,
    EarlyHints = 103,

    // 2xx Success
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NonAuthoritativeInformation = 203,
    NoContent = 204,
    ResetContent = 205,
    PartialContent = 206,
    MultiStatus = 207,
    AlreadyReported = 208,
    IMUsed = 226,

    // 3xx Redirection
    MultipleChoices = 300,
    MovedPermanently = 301,
    Found = 302,
    SeeOther = 303,
    NotModified = 304,
    UseProxy = 305,
    TemporaryRedirect = 307,
    PermanentRedirect = 308,

    // 4xx Client Error
    BadRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    ProxyAuthenticationRequired = 407,
    RequestTimeout = 408,
    Conflict = 409,
    Gone = 410,
    LengthRequired = 411,
    PreconditionFailed = 412,
    PayloadTooLarge = 413,
    URITooLong = 414,
    UnsupportedMediaType = 415,
    RangeNotSatisfiable = 416,
    ExpectationFailed = 417,
    ImATeapot = 418,
    MisdirectedRequest = 421,
    UnprocessableEntity = 422,
    Locked = 423,
    FailedDependency = 424,
    TooEarly = 425,
    UpgradeRequired = 426,
    PreconditionRequired = 428,
    TooManyRequests = 429,
    RequestHeaderFieldsTooLarge = 431,
    UnavailableForLegalReasons = 451,

    // 5xx Server Error
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
    HTTPVersionNotSupported = 505,
    VariantAlsoNegotiates = 506,
    InsufficientStorage = 507,
    LoopDetected = 508,
    NotExtended = 510,
    NetworkAuthenticationRequired = 511,
}

impl StatusCode {
    /// Get the reason phrase for the status code
    pub fn reason_phrase(&self) -> &'static str {
        match self {
            // 1xx
            StatusCode::Continue => "Continue",
            StatusCode::SwitchingProtocols => "Switching Protocols",
            StatusCode::Processing => "Processing",
            StatusCode::EarlyHints => "Early Hints",

            // 2xx
            StatusCode::Ok => "OK",
            StatusCode::Created => "Created",
            StatusCode::Accepted => "Accepted",
            StatusCode::NonAuthoritativeInformation => "Non-Authoritative Information",
            StatusCode::NoContent => "No Content",
            StatusCode::ResetContent => "Reset Content",
            StatusCode::PartialContent => "Partial Content",
            StatusCode::MultiStatus => "Multi-Status",
            StatusCode::AlreadyReported => "Already Reported",
            StatusCode::IMUsed => "IM Used",

            // 3xx
            StatusCode::MultipleChoices => "Multiple Choices",
            StatusCode::MovedPermanently => "Moved Permanently",
            StatusCode::Found => "Found",
            StatusCode::SeeOther => "See Other",
            StatusCode::NotModified => "Not Modified",
            StatusCode::UseProxy => "Use Proxy",
            StatusCode::TemporaryRedirect => "Temporary Redirect",
            StatusCode::PermanentRedirect => "Permanent Redirect",

            // 4xx
            StatusCode::BadRequest => "Bad Request",
            StatusCode::Unauthorized => "Unauthorized",
            StatusCode::PaymentRequired => "Payment Required",
            StatusCode::Forbidden => "Forbidden",
            StatusCode::NotFound => "Not Found",
            StatusCode::MethodNotAllowed => "Method Not Allowed",
            StatusCode::NotAcceptable => "Not Acceptable",
            StatusCode::ProxyAuthenticationRequired => "Proxy Authentication Required",
            StatusCode::RequestTimeout => "Request Timeout",
            StatusCode::Conflict => "Conflict",
            StatusCode::Gone => "Gone",
            StatusCode::LengthRequired => "Length Required",
            StatusCode::PreconditionFailed => "Precondition Failed",
            StatusCode::PayloadTooLarge => "Payload Too Large",
            StatusCode::URITooLong => "URI Too Long",
            StatusCode::UnsupportedMediaType => "Unsupported Media Type",
            StatusCode::RangeNotSatisfiable => "Range Not Satisfiable",
            StatusCode::ExpectationFailed => "Expectation Failed",
            StatusCode::ImATeapot => "I'm a teapot",
            StatusCode::MisdirectedRequest => "Misdirected Request",
            StatusCode::UnprocessableEntity => "Unprocessable Entity",
            StatusCode::Locked => "Locked",
            StatusCode::FailedDependency => "Failed Dependency",
            StatusCode::TooEarly => "Too Early",
            StatusCode::UpgradeRequired => "Upgrade Required",
            StatusCode::PreconditionRequired => "Precondition Required",
            StatusCode::TooManyRequests => "Too Many Requests",
            StatusCode::RequestHeaderFieldsTooLarge => "Request Header Fields Too Large",
            StatusCode::UnavailableForLegalReasons => "Unavailable For Legal Reasons",

            // 5xx
            StatusCode::InternalServerError => "Internal Server Error",
            StatusCode::NotImplemented => "Not Implemented",
            StatusCode::BadGateway => "Bad Gateway",
            StatusCode::ServiceUnavailable => "Service Unavailable",
            StatusCode::GatewayTimeout => "Gateway Timeout",
            StatusCode::HTTPVersionNotSupported => "HTTP Version Not Supported",
            StatusCode::VariantAlsoNegotiates => "Variant Also Negotiates",
            StatusCode::InsufficientStorage => "Insufficient Storage",
            StatusCode::LoopDetected => "Loop Detected",
            StatusCode::NotExtended => "Not Extended",
            StatusCode::NetworkAuthenticationRequired => "Network Authentication Required",
        }
        
    }

    /// Check if the status code is informational (1xx)
    pub fn is_informational(&self) -> bool {
        (*self as u16) >= 100 && (*self as u16) < 200
    }

    /// Check if the status code is successful (2xx)
    pub fn is_success(&self) -> bool {
        (*self as u16) >= 200 && (*self as u16) < 300
    }

    /// Check if the status code is a redirection (3xx)
    pub fn is_redirection(&self) -> bool {
        (*self as u16) >= 300 && (*self as u16) < 400
    }

    /// Check if the status code is a client error (4xx)
    pub fn is_client_error(&self) -> bool {
        (*self as u16) >= 400 && (*self as u16) < 500
    }

    /// Check if the status code is a server error (5xx)
    pub fn is_server_error(&self) -> bool {
        (*self as u16) >= 500 && (*self as u16) < 600
    }

    /// Check if the status code is an error (4xx or 5xx)
    pub fn is_error(&self) -> bool {
        self.is_client_error() || self.is_server_error()
    }

    /// Convert from u16 to StatusCode
    pub fn from_u16(code: u16) -> Result<Self, InvalidStatusCode> {
        match code {
            // 1xx
            100 => Ok(StatusCode::Continue),
            101 => Ok(StatusCode::SwitchingProtocols),
            102 => Ok(StatusCode::Processing),
            103 => Ok(StatusCode::EarlyHints),

            // 2xx
            200 => Ok(StatusCode::Ok),
            201 => Ok(StatusCode::Created),
            202 => Ok(StatusCode::Accepted),
            203 => Ok(StatusCode::NonAuthoritativeInformation),
            204 => Ok(StatusCode::NoContent),
            205 => Ok(StatusCode::ResetContent),
            206 => Ok(StatusCode::PartialContent),
            207 => Ok(StatusCode::MultiStatus),
            208 => Ok(StatusCode::AlreadyReported),
            226 => Ok(StatusCode::IMUsed),

            // 3xx
            300 => Ok(StatusCode::MultipleChoices),
            301 => Ok(StatusCode::MovedPermanently),
            302 => Ok(StatusCode::Found),
            303 => Ok(StatusCode::SeeOther),
            304 => Ok(StatusCode::NotModified),
            305 => Ok(StatusCode::UseProxy),
            307 => Ok(StatusCode::TemporaryRedirect),
            308 => Ok(StatusCode::PermanentRedirect),

            // 4xx
            400 => Ok(StatusCode::BadRequest),
            401 => Ok(StatusCode::Unauthorized),
            402 => Ok(StatusCode::PaymentRequired),
            403 => Ok(StatusCode::Forbidden),
            404 => Ok(StatusCode::NotFound),
            405 => Ok(StatusCode::MethodNotAllowed),
            406 => Ok(StatusCode::NotAcceptable),
            407 => Ok(StatusCode::ProxyAuthenticationRequired),
            408 => Ok(StatusCode::RequestTimeout),
            409 => Ok(StatusCode::Conflict),
            410 => Ok(StatusCode::Gone),
            411 => Ok(StatusCode::LengthRequired),
            412 => Ok(StatusCode::PreconditionFailed),
            413 => Ok(StatusCode::PayloadTooLarge),
            414 => Ok(StatusCode::URITooLong),
            415 => Ok(StatusCode::UnsupportedMediaType),
            416 => Ok(StatusCode::RangeNotSatisfiable),
            417 => Ok(StatusCode::ExpectationFailed),
            418 => Ok(StatusCode::ImATeapot),
            421 => Ok(StatusCode::MisdirectedRequest),
            422 => Ok(StatusCode::UnprocessableEntity),
            423 => Ok(StatusCode::Locked),
            424 => Ok(StatusCode::FailedDependency),
            425 => Ok(StatusCode::TooEarly),
            426 => Ok(StatusCode::UpgradeRequired),
            428 => Ok(StatusCode::PreconditionRequired),
            429 => Ok(StatusCode::TooManyRequests),
            431 => Ok(StatusCode::RequestHeaderFieldsTooLarge),
            451 => Ok(StatusCode::UnavailableForLegalReasons),

            // 5xx
            500 => Ok(StatusCode::InternalServerError),
            501 => Ok(StatusCode::NotImplemented),
            502 => Ok(StatusCode::BadGateway),
            503 => Ok(StatusCode::ServiceUnavailable),
            504 => Ok(StatusCode::GatewayTimeout),
            505 => Ok(StatusCode::HTTPVersionNotSupported),
            506 => Ok(StatusCode::VariantAlsoNegotiates),
            507 => Ok(StatusCode::InsufficientStorage),
            508 => Ok(StatusCode::LoopDetected),
            510 => Ok(StatusCode::NotExtended),
            511 => Ok(StatusCode::NetworkAuthenticationRequired),

            _ => Err(InvalidStatusCode(code)),
        }
    }

    /// Get the numeric value of the status code
    pub fn as_u16(&self) -> u16 {
        *self as u16
    }
}

impl std::fmt::Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.as_u16(), self.reason_phrase())
    }
}

impl TryFrom<u16> for StatusCode {
    type Error = InvalidStatusCode;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        StatusCode::from_u16(value)
    }
}

impl From<StatusCode> for u16 {
    fn from(code: StatusCode) -> Self {
        code as u16
    }
}

// Error type for invalid status codes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidStatusCode(pub u16);

impl std::fmt::Display for InvalidStatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid status code: {}", self.0)
    }
}

impl std::error::Error for InvalidStatusCode {}

// #[derive(Debug, Clone, PartialEq)]
// pub enum StatusCode {
//     Ok = 200,
//     Created = 201,
//     NoContent = 204,
//     BadRequest = 400,
//     Unauthorized = 401,
//     Forbidden = 403,
//     NotFound = 404,
//     InternalServerError = 500,
//     NotImplemented = 501,
//     ServiceUnavailable = 503,
// }

// impl Display for StatusCode {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         let (code, reason) = match self {
//             StatusCode::Ok => (200, "OK"),
//             StatusCode::Created => (201, "Created"),
//             StatusCode::NoContent => (204, "No Content"),
//             StatusCode::BadRequest => (400, "Bad Request"),
//             StatusCode::Unauthorized => (401, "Unauthorized"),
//             StatusCode::Forbidden => (403, "Forbidden"),
//             StatusCode::NotFound => (404, "Not Found"),
//             StatusCode::InternalServerError => (500, "Internal Server Error"),
//             StatusCode::NotImplemented => (501, "Not Implemented"),
//             StatusCode::ServiceUnavailable => (503, "Service Unavailable"),
//         };
//         write!(f, "{} {}", code, reason)
//     }
// }

#[derive(Debug, Clone)]
pub struct HttpResponse {
    version: HttpVersion,
    status: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl HttpResponse {
    pub fn new() -> Self {
        Self {
            version: HttpVersion::Http1_1,
            status: StatusCode::Ok,
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn version(mut self, version: HttpVersion) -> Self {
        self.version = version;
        self
    }

    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_body<T: Into<String>>(body: T) -> Self {
        let body_str = body.into();
        let mut headers = HashMap::new();
        headers.insert("Content-Length".to_string(), body_str.len().to_string());
        
        Self {
            status: StatusCode::Ok,
            headers,
            body: Some(body_str),
            version: HttpVersion::Http1_1,
        }
    }

    pub fn with_json<T: Serialize>(data: T) -> Result<Self, serde_json::Error> {
        let body = serde_json::to_string(&data)?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Content-Length".to_string(), body.len().to_string());

        Ok(Self {
            status: StatusCode::Ok,
            headers,
            body: Some(body),
            version: HttpVersion::Http1_1,
        })
    }

    pub fn content_type(mut self, content_type: &str) -> Self {
        self.headers.insert("Content-Type".to_string(), content_type.to_string());
        self
    }

    pub fn body<T: Into<String>>(mut self, body: T) -> Self {
        let body_str = body.into();
        self.headers.insert("Content-Length".to_string(), body_str.len().to_string());
        self.body = Some(body_str);
        self
    }

    pub fn json<T: serde::Serialize>(mut self, data: T) -> Result<Self, serde_json::Error> {
        let json_string = serde_json::to_string(&data)?;
        self.headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.headers.insert("Content-Length".to_string(), json_string.len().to_string());
        self.body = Some(json_string);
        Ok(self)
    }

    pub fn build(self) -> String {
        let status_line = format!("{} {}", self.version, self.status);
        
        let headers: Vec<String> = self.headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect();
        
        let headers_section = if headers.is_empty() {
            String::new()
        } else {
            format!("{}\r\n", headers.join("\r\n"))
        };

        let body_section = match self.body {
            Some(body) => format!("\r\n{}", body),
            None => String::new(),
        };

        format!("{}\r\n{}{}", status_line, headers_section, body_section)
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpResponse {
    // Success responses
    pub fn ok() -> Self {
        Self::new().status(StatusCode::Ok)
    }

    pub fn created() -> Self {
        Self::new().status(StatusCode::Created)
    }

    pub fn no_content() -> Self {
        Self::new()
            .status(StatusCode::NoContent)
            .header("Content-Length", "0")
    }

    // Error responses
    pub fn bad_request() -> Self {
        Self::new().status(StatusCode::BadRequest)
    }

    pub fn not_found() -> Self {
        Self::new().status(StatusCode::NotFound)
    }

    pub fn internal_server_error() -> Self {
        Self::new().status(StatusCode::InternalServerError)
    }

    // Common content types
    pub fn html<T: Into<String>>(self, content: T) -> Self {
        self.content_type("text/html; charset=utf-8").body(content)
    }

    pub fn text<T: Into<String>>(self, content: T) -> Self {
        self.content_type("text/plain; charset=utf-8").body(content)
    }

    pub fn json_body<T: serde::Serialize>(self, data: T) -> Result<Self, serde_json::Error> {
        self.json(data)
    }
}