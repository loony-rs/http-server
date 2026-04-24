# Middleware

## Overview

Middleware intercepts every request before it reaches a handler and every response before it is sent to the client. The execution model is an **onion**: the first `.wrap()` call wraps the outermost layer, running first on the request path and last on the response path.

```
request  ──► Middleware A ──► Middleware B ──► Handler
response ◄── Middleware A ◄── Middleware B ◄── Handler
```

Registration order:
```rust
App::new()
    .wrap(A)   // outermost — runs first on request, last on response
    .wrap(B)   // inner — runs second on request, first on response
    .routes(routes)
```

## The Middleware Trait

```rust
pub trait Middleware: 'static {
    fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse>;
}
```

- `req` — the incoming request
- `next` — the rest of the chain; call `next.call(req).await` to proceed
- Return a `BoxFuture<ServiceResponse>` (a pinned, boxed async block)

## Built-in Middleware

### Logger

Logs each request as a structured `tracing::info!` event.

```rust
use loony_server::middleware::Logger;

App::new().wrap(Logger).routes(routes)
```

Output (with `RUST_LOG=info`):

```
INFO loony_server::server: request peer="127.0.0.1:54321" method="GET" path="/users" status="200" latency_ms=3
```

Fields logged: `peer`, `method`, `path`, `status`, `latency_ms`.

### Cors

Adds CORS headers to every response and handles `OPTIONS` preflight requests.

```rust
use loony_server::middleware::Cors;

// Permissive (allow-all)
App::new().wrap(Cors::new()).routes(routes)

// Restricted origin
App::new()
    .wrap(
        Cors::new()
            .allow_origin("https://example.com")
            .allow_methods("GET, POST, PUT, DELETE")
            .allow_headers("Content-Type, Authorization")
            .max_age(7200),
    )
    .routes(routes)
```

| Builder method | Default |
|---|---|
| `.allow_origin(str)` | `*` |
| `.allow_methods(str)` | `GET, POST, PUT, DELETE, OPTIONS, PATCH` |
| `.allow_headers(str)` | `Content-Type, Authorization, Accept` |
| `.max_age(u32)` | `3600` |

`OPTIONS` preflight requests are short-circuited with `204 No Content` — the handler is never called.

## Writing Custom Middleware

```rust
use loony_server::middleware::{BoxFuture, Middleware, Next};
use loony_server::service::{ServiceRequest, ServiceResponse};
use loony_server::response::HttpResponse;

pub struct ApiKeyGuard {
    required_key: String,
}

impl Middleware for ApiKeyGuard {
    fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse> {
        let required = self.required_key.clone();

        Box::pin(async move {
            let has_key = req.req.headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("x-api-key") && v == &required);

            if !has_key {
                return ServiceResponse(
                    HttpResponse::new()
                        .with_status(loony_server::response::StatusCode::Unauthorized)
                        .body("missing or invalid API key")
                        .build(),
                );
            }

            next.call(req).await
        })
    }
}
```

Register it:

```rust
App::new()
    .wrap(Logger)
    .wrap(ApiKeyGuard { required_key: "secret123".into() })
    .routes(routes)
```

## Request Timing Example

```rust
use std::time::Instant;

pub struct Timer;

impl Middleware for Timer {
    fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse> {
        Box::pin(async move {
            let start = Instant::now();
            let resp = next.call(req).await;
            let ms = start.elapsed().as_millis();
            // Inject the timing header into the already-serialised response.
            // (A real implementation would use inject_headers or a response wrapper.)
            tracing::debug!(latency_ms = ms, "handler timing");
            resp
        })
    }
}
```

## Short-Circuiting

A middleware can return a response without calling `next.call()`. This is used by `Cors` for preflight, but also useful for authentication, rate-limiting, etc.:

```rust
impl Middleware for RateLimit {
    fn handle(&self, req: ServiceRequest, next: Next) -> BoxFuture<ServiceResponse> {
        Box::pin(async move {
            if self.is_limited() {
                return ServiceResponse(
                    HttpResponse::new()
                        .with_status(StatusCode::TooManyRequests)
                        .body("rate limit exceeded")
                        .build(),
                );
            }
            next.call(req).await
        })
    }
}
```

## Accessing Request Data in Middleware

`ServiceRequest` exposes:

```rust
pub struct ServiceRequest {
    pub req: HttpRequest,         // method, uri, version, headers, body
    pub extensions: Rc<Extensions>, // shared app data
    pub path_params: Rc<Vec<String>>, // extracted path parameters
}
```

```rust
let method = req.req.method.as_deref().unwrap_or("-");
let path   = req.req.uri.as_deref().unwrap_or("-");
let headers = &req.req.headers; // Vec<(String, String)>
```

## Middleware Execution Order — Detail

Given:
```rust
App::new().wrap(A).wrap(B).routes(routes)
```

The chain is built as `A(B(handler))`. Call order:
1. A receives request → calls `next.call(req)`
2. B receives request → calls `next.call(req)`
3. Handler runs → produces response
4. B receives response → returns it (possibly modified)
5. A receives response → returns it (possibly modified)
