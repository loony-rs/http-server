use std::{collections::HashMap, future::{Future, Ready, ready}};
use futures::executor::block_on;

use crate::{response::{HttpResponse, StatusCode}, service::{ServiceRequest, ServiceResponse}};

pub trait Responder {
    type Future: Future<Output=ServiceResponse>;
    fn respond(&self) -> Self::Future;
}

// Implement Responder for String
impl Responder for String {
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let response = HttpResponse::new().body(self.clone()).build();
        ready(ServiceResponse(response))
    }
}

// Implement Responder for &str (avoid cloning when possible)
impl Responder for &str {
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let response = HttpResponse::new().body(self.to_string()).build();
        ready(ServiceResponse(response))
    }
}

// Implement Responder for Result<String, E>
impl<E> Responder for Result<String, E>
where
    E: std::fmt::Display,
{
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        match self {
            Ok(success) => {
                let response = HttpResponse::new().body(success.clone()).build();
                ready(ServiceResponse(response))
            }
            Err(error) => {
                let response = HttpResponse::new().body(error.to_string())
                    .with_status(StatusCode::InternalServerError).build();
                ready(ServiceResponse(response))
            }
        }
    }
}

// Implement Responder for HttpResponse
impl Responder for HttpResponse {
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        ready(ServiceResponse(self.clone().build()))
    }
}

// Implement Responder for Vec<u8> (binary data)
impl Responder for Vec<u8> {
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        // Convert bytes to string (you might want to handle this differently)
        let body = String::from_utf8_lossy(self).to_string();
        let mut response = HttpResponse::new().body(body);
        response.headers.insert("Content-Type".to_string(), "application/octet-stream".to_string());
        let response = response.build();
        ready(ServiceResponse(response))
    }
}

// Implement Responder for &[u8] (binary data slice)
impl Responder for &[u8] {
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let body = String::from_utf8_lossy(self).to_string();
        let mut response = HttpResponse::new().body(body);
        response.headers.insert("Content-Type".to_string(), "application/octet-stream".to_string());
        let response = response.build();
        ready(ServiceResponse(response))
    }
}

// Implement Responder for any Serializable type (JSON responses)
// impl<T> Responder for T
// where
//     T: Serialize,
// {
//     type Future = Ready<ServiceResponse>;

//     fn respond(&self) -> Self::Future {
//         match HttpResponse::with_json(self) {
//             Ok(response) => ready(ServiceResponse(response)),
//             Err(error) => {
//                 let error_response = HttpResponse::with_body(format!("Serialization error: {}", error))
//                     .with_status(500);
//                 ready(ServiceResponse(error_response))
//             }
//         }
//     }
// }

// Implement Responder for Option<T>
// impl<T> Responder for Option<T>
// where
//     T: Responder,
// {
//     type Future = Ready<ServiceResponse>;

//     fn respond(&self, req: &ServiceRequest) -> Self::Future {
//         match self {
//             Some(inner) => ready(block_on(inner.respond(req))),
//             None => {
//                 let response = HttpResponse::new().body("Not Found")
//                     .with_status(StatusCode::NotFound).build();
//                 ready(ServiceResponse(response))
//             }
//         }
//     }
// }

// Implement Responder for tuples (Status, Body)
impl<T> Responder for (StatusCode, T)
where
    T: Into<String>,
    for<'a> &'a T: Into<String>,
{
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let (status, body) = self;
        let response = HttpResponse::with_body(body)
            .with_status(*status).build();
        ready(ServiceResponse(response))
    }
}

// Implement Responder for tuples (Status, Headers, Body)
impl<T> Responder for (StatusCode, HashMap<String, String>, T)
where
    T: Into<String>,
    for<'a> &'a T: Into<String>,
{
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let (status, headers, body) = self;
        let mut response = HttpResponse::with_body(body)
            .with_status(*status);
        response.headers.extend(headers.clone());
        let response = response.build();
        ready(ServiceResponse(response))
    }
}

// Implement Responder for () - empty response
impl Responder for () {
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let response = HttpResponse::new().build();
        ready(ServiceResponse(response))
    }
}

// Custom type for redirects
pub struct Redirect(pub String);

impl Responder for Redirect {
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let mut response = HttpResponse::new()
            .with_status(StatusCode::Found)
            .with_header("Location", &self.0);
        response.body = Some(format!("Redirecting to {}", self.0));
        let response = response.build();
        ready(ServiceResponse(response))
    }
}

// Custom type for HTML responses
pub struct Html<T>(pub T);

impl<T> Responder for Html<T>
where
    T: Into<String> + Clone,
{
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let response = HttpResponse::with_body(self.0.clone())
            .with_header("Content-Type", "text/html; charset=utf-8").build();
        ready(ServiceResponse(response))
    }
}

// Custom type for plain text responses
pub struct Text<T>(pub T);

impl<T> Responder for Text<T>
where
    T: Into<String> + Clone,
{
    type Future = Ready<ServiceResponse>;

    fn respond(&self) -> Self::Future {
        let response = HttpResponse::with_body(self.0.clone())
            .with_header("Content-Type", "text/plain; charset=utf-8").build();
        ready(ServiceResponse(response))
    }
}