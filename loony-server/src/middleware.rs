use crate::service::{ServiceRequest, ServiceResponse};
use std::{future::Future, pin::Pin, rc::Rc, time::Instant};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T>>>;

/// Carries the remaining middleware chain.
///
/// Calling `.call(req)` invokes the next service — either another middleware
/// or the final route handler — and returns its response.
pub struct Next {
    run: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>>,
}

impl Next {
    pub(crate) fn new(run: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>>) -> Self {
        Next { run }
    }

    pub async fn call(self, req: ServiceRequest) -> ServiceResponse {
        (self.run)(req).await
    }
}

/// Implement this trait to create middleware.
///
/// Middlewares are registered via `App::wrap()`.  The first registered
/// middleware is the outermost wrapper: it runs first on the request path
/// and last on the response path.
///
/// # Example
///
/// ```rust,ignore
/// use loony_server::middleware::{BoxFuture, Middleware, Next};
/// use loony_server::service::{ServiceRequest, ServiceResponse};
///
/// pub struct Logger;
///
/// impl Middleware for Logger {
///     fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse> {
///         Box::pin(async move {
///             let path = req.req.uri.clone().unwrap_or_default();
///             let resp = next.call(req).await;
///             println!("GET {path}");
///             resp
///         })
///     }
/// }
/// ```
pub trait Middleware: 'static {
    fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse>;
}

// ---------------------------------------------------------------------------
// Logger
// ---------------------------------------------------------------------------

/// Logs each request as `METHOD /path STATUS elapsed_ms`.
///
/// # Usage
/// ```rust,ignore
/// App::new().wrap(Logger).routes(routes)
/// ```
pub struct Logger;

impl Middleware for Logger {
    fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse> {
        Box::pin(async move {
            let method = req.req.method.clone().unwrap_or_else(|| "-".to_string());
            let path = req.req.uri.clone().unwrap_or_else(|| "-".to_string());
            let start = Instant::now();

            let resp = next.call(req).await;

            // Parse the status code from the serialised response first line.
            let status = resp
                .0
                .split("\r\n")
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("-")
                .to_string();

            println!("{method} {path} {status} {}ms", start.elapsed().as_millis());
            resp
        })
    }
}

// ---------------------------------------------------------------------------
// Cors
// ---------------------------------------------------------------------------

/// Adds CORS response headers and handles `OPTIONS` preflight requests.
///
/// # Usage
/// ```rust,ignore
/// App::new()
///     .wrap(Cors::new())
///     .wrap(Cors::new().allow_origin("https://example.com"))
///     .routes(routes)
/// ```
pub struct Cors {
    allow_origin: String,
    allow_methods: String,
    allow_headers: String,
    max_age: u32,
}

impl Cors {
    pub fn new() -> Self {
        Cors {
            allow_origin: "*".to_string(),
            allow_methods: "GET, POST, PUT, DELETE, OPTIONS, PATCH".to_string(),
            allow_headers: "Content-Type, Authorization, Accept".to_string(),
            max_age: 3600,
        }
    }

    pub fn allow_origin(mut self, origin: &str) -> Self {
        self.allow_origin = origin.to_string();
        self
    }

    pub fn allow_methods(mut self, methods: &str) -> Self {
        self.allow_methods = methods.to_string();
        self
    }

    pub fn allow_headers(mut self, headers: &str) -> Self {
        self.allow_headers = headers.to_string();
        self
    }

    pub fn max_age(mut self, seconds: u32) -> Self {
        self.max_age = seconds;
        self
    }
}

impl Default for Cors {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for Cors {
    fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse> {
        let is_preflight = req.req.method.as_deref() == Some("OPTIONS");
        let allow_origin = self.allow_origin.clone();
        let allow_methods = self.allow_methods.clone();
        let allow_headers = self.allow_headers.clone();
        let max_age = self.max_age;

        Box::pin(async move {
            if is_preflight {
                // Short-circuit: no need to call the handler for preflight.
                let resp = format!(
                    "HTTP/1.1 204 No Content\r\n\
                     Access-Control-Allow-Origin: {allow_origin}\r\n\
                     Access-Control-Allow-Methods: {allow_methods}\r\n\
                     Access-Control-Allow-Headers: {allow_headers}\r\n\
                     Access-Control-Max-Age: {max_age}\r\n\
                     Content-Length: 0\r\n\
                     \r\n"
                );
                return ServiceResponse(resp);
            }

            let mut resp = next.call(req).await;
            resp.0 = inject_headers(
                resp.0,
                &[
                    ("Access-Control-Allow-Origin", &allow_origin),
                    ("Access-Control-Allow-Methods", &allow_methods),
                    ("Access-Control-Allow-Headers", &allow_headers),
                ],
            );
            resp
        })
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Insert `(name, value)` header pairs into an already-serialised HTTP
/// response, immediately before the blank line that separates headers from
/// the body.
fn inject_headers(mut response: String, headers: &[(&str, &str)]) -> String {
    let Some(header_end) = response.find("\r\n\r\n") else {
        return response;
    };
    let mut block = String::new();
    for (name, value) in headers {
        block.push_str(&format!("{name}: {value}\r\n"));
    }
    // Insert after the last header's \r\n, before the blank line.
    response.insert_str(header_end + 2, &block);
    response
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{extensions::Extensions, request::HttpRequest};
    use futures::executor::block_on;
    use std::rc::Rc;

    fn make_req() -> ServiceRequest {
        ServiceRequest {
            req: HttpRequest::new(),
            extensions: Rc::new(Extensions::new()),
            path_params: Rc::new(vec![]),
        }
    }

    /// Appends a fixed tag to the response body so we can check execution order.
    struct Tag(&'static str);

    impl Middleware for Tag {
        fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse> {
            let tag = self.0;
            Box::pin(async move {
                let mut resp = next.call(req).await;
                resp.0.push_str(tag);
                resp
            })
        }
    }

    /// Builds the middleware chain the same way `Run::call_service` does.
    fn build_chain(
        middlewares: &[Rc<dyn Middleware>],
        terminal: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>>,
    ) -> Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> {
        middlewares.iter().rev().fold(terminal, |inner, mw| {
            let mw = Rc::clone(mw);
            Rc::new(move |req: ServiceRequest| {
                let next = Next::new(Rc::clone(&inner));
                mw.handle(req, next)
            }) as Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>>
        })
    }

    #[test]
    fn no_middleware_passes_through() {
        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> =
            Rc::new(|_req| Box::pin(async { ServiceResponse("ok".to_string()) }));

        let chain = build_chain(&[], Rc::clone(&base));
        let resp = block_on(chain(make_req()));
        assert_eq!(resp.0, "ok");
    }

    #[test]
    fn single_middleware_wraps_handler() {
        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> =
            Rc::new(|_req| Box::pin(async { ServiceResponse("ok".to_string()) }));

        let mws: Vec<Rc<dyn Middleware>> = vec![Rc::new(Tag("X"))];
        let chain = build_chain(&mws, base);
        let resp = block_on(chain(make_req()));
        assert_eq!(resp.0, "okX");
    }

    #[test]
    fn middleware_execution_order() {
        // App::new().wrap(A).wrap(B) stores [A, B].
        // A is outermost: runs first on request, last on response.
        // Expected response tag order: base + B's tag + A's tag = "okBA"
        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> =
            Rc::new(|_req| Box::pin(async { ServiceResponse("ok".to_string()) }));

        let mws: Vec<Rc<dyn Middleware>> = vec![Rc::new(Tag("A")), Rc::new(Tag("B"))];
        let chain = build_chain(&mws, base);
        let resp = block_on(chain(make_req()));
        assert_eq!(resp.0, "okBA");
    }

    #[test]
    fn middleware_can_short_circuit() {
        struct Block;
        impl Middleware for Block {
            fn handle(&self, _req: ServiceRequest, _next: Next) -> BoxFuture<ServiceResponse> {
                Box::pin(async { ServiceResponse("blocked".to_string()) })
            }
        }

        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> =
            Rc::new(|_req| Box::pin(async { ServiceResponse("handler".to_string()) }));

        let mws: Vec<Rc<dyn Middleware>> = vec![Rc::new(Block)];
        let chain = build_chain(&mws, base);
        let resp = block_on(chain(make_req()));
        assert_eq!(resp.0, "blocked");
    }

    // --- inject_headers helper ---

    #[test]
    fn inject_headers_inserts_before_blank_line() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi".to_string();
        let out = inject_headers(resp, &[("X-Foo", "bar")]);
        assert!(out.contains("X-Foo: bar\r\n"));
        assert!(out.ends_with("\r\n\r\nhi"));
    }

    #[test]
    fn inject_headers_multiple() {
        let resp = "HTTP/1.1 200 OK\r\n\r\n".to_string();
        let out = inject_headers(resp, &[("A", "1"), ("B", "2")]);
        assert!(out.contains("A: 1\r\n"));
        assert!(out.contains("B: 2\r\n"));
        assert!(out.ends_with("\r\n\r\n"));
    }

    // --- Logger ---

    #[test]
    fn logger_passes_response_through_unchanged() {
        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> = Rc::new(|_req| {
            Box::pin(async {
                ServiceResponse("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi".to_string())
            })
        });

        let mws: Vec<Rc<dyn Middleware>> = vec![Rc::new(Logger)];
        let chain = build_chain(&mws, base);
        let resp = block_on(chain(make_req()));
        assert!(resp.0.contains("HTTP/1.1 200 OK"));
        assert!(resp.0.ends_with("hi"));
    }

    // --- Cors ---

    #[test]
    fn cors_adds_headers_to_regular_response() {
        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> = Rc::new(|_req| {
            Box::pin(async {
                ServiceResponse("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi".to_string())
            })
        });

        let mws: Vec<Rc<dyn Middleware>> = vec![Rc::new(Cors::new())];
        let chain = build_chain(&mws, base);
        let resp = block_on(chain(make_req()));
        assert!(resp.0.contains("Access-Control-Allow-Origin: *\r\n"));
        assert!(resp.0.contains("Access-Control-Allow-Methods:"));
        assert!(resp.0.ends_with("hi"));
    }

    #[test]
    fn cors_preflight_short_circuits_with_204() {
        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> =
            Rc::new(|_req| Box::pin(async { ServiceResponse("should not reach".to_string()) }));

        let mut req = make_req();
        req.req.method = Some("OPTIONS".to_string());

        let mws: Vec<Rc<dyn Middleware>> = vec![Rc::new(Cors::new())];
        let chain = build_chain(&mws, base);
        let resp = block_on(chain(req));
        assert!(resp.0.contains("HTTP/1.1 204 No Content"));
        assert!(resp.0.contains("Access-Control-Allow-Origin: *"));
        assert!(!resp.0.contains("should not reach"));
    }

    #[test]
    fn cors_custom_origin() {
        let base: Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>> = Rc::new(|_req| {
            Box::pin(async {
                ServiceResponse("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_string())
            })
        });

        let mws: Vec<Rc<dyn Middleware>> =
            vec![Rc::new(Cors::new().allow_origin("https://example.com"))];
        let chain = build_chain(&mws, base);
        let resp = block_on(chain(make_req()));
        assert!(resp.0.contains("Access-Control-Allow-Origin: https://example.com\r\n"));
    }
}
