# Responses

## The Responder Trait

Any type that implements `Responder` can be returned from a handler:

```rust
pub trait Responder {
    type Future: Future<Output = ServiceResponse>;
    fn respond(&self) -> Self::Future;
}
```

### Built-in Responder implementations

| Return type | Behaviour |
|---|---|
| `String` | 200 OK with plain-text body |
| `&str` | 200 OK with plain-text body |
| `HttpResponse` | full control over status, headers, body |
| `(StatusCode, T)` | custom status + body |
| `(StatusCode, HashMap<String,String>, T)` | custom status + headers + body |
| `Html<T>` | 200 OK with `Content-Type: text/html` |
| `Text<T>` | 200 OK with `Content-Type: text/plain` |
| `Redirect(url)` | 302 Found with `Location` header |
| `Vec<u8>` | 200 OK with `Content-Type: application/octet-stream` |
| `Result<String, E>` | 200 on Ok, 500 on Err |
| `()` | 200 OK with empty body |

### Examples

```rust
use loony_server::responder::{Html, Redirect, Text};
use loony_server::response::{HttpResponse, StatusCode};
use std::collections::HashMap;

// Plain string
async fn hello() -> impl Responder { "Hello!" }

// Status + body
async fn gone() -> impl Responder {
    (StatusCode::Gone, "this resource no longer exists")
}

// HTML
async fn page() -> impl Responder {
    Html("<h1>Hello</h1>")
}

// Redirect
async fn old_path() -> impl Responder {
    Redirect("/new-path".into())
}

// Full control
async fn custom() -> impl Responder {
    HttpResponse::ok()
        .header("X-Custom", "value")
        .body("response body")
}
```

## HttpResponse

`HttpResponse` is the builder for full responses. Call `.build()` to produce the serialised HTTP string.

### Constructors

```rust
use loony_server::response::{HttpResponse, StatusCode};

HttpResponse::new()                     // 200 OK, no body
HttpResponse::ok()                      // 200 OK
HttpResponse::created()                 // 201 Created
HttpResponse::no_content()              // 204 No Content
HttpResponse::bad_request()             // 400 Bad Request
HttpResponse::not_found()               // 404 Not Found
HttpResponse::internal_server_error()   // 500 Internal Server Error

// With a pre-set body
HttpResponse::with_body("hello")        // 200 OK + body + Content-Length
HttpResponse::with_json(&value).unwrap()// 200 OK + JSON body + Content-Type
```

### Builder methods

```rust
let response = HttpResponse::new()
    .status(StatusCode::Created)
    .header("X-Trace-Id", "abc123")
    .content_type("application/json")
    .body(r#"{"id":1}"#)
    .build();
```

| Method | Description |
|---|---|
| `.status(StatusCode)` | set status code |
| `.with_status(StatusCode)` | alias for `.status()` |
| `.header(key, value)` | add a response header (CR/LF stripped) |
| `.with_header(key, value)` | alias for `.header()` |
| `.content_type(str)` | set `Content-Type` |
| `.body(T)` | set body; auto-sets `Content-Length` |
| `.html(T)` | set body + `Content-Type: text/html` |
| `.text(T)` | set body + `Content-Type: text/plain` |
| `.json(T)` | set JSON body + `Content-Type: application/json` |
| `.with_json(T)` | constructor-style: returns `Result<Self, _>` |
| `.build()` | serialise to HTTP response string |

### Content-Length is always correct

`build()` removes any caller-provided `Content-Length` and recalculates it from the actual body. You cannot accidentally send a mismatched `Content-Length`.

### Header injection protection

`.header()` and `.with_header()` strip `\r` and `\n` from both the name and value, preventing response splitting attacks.

## StatusCode

All standard HTTP status codes are available as enum variants:

```rust
use loony_server::response::StatusCode;

StatusCode::Ok                   // 200
StatusCode::Created              // 201
StatusCode::NoContent            // 204
StatusCode::MovedPermanently     // 301
StatusCode::Found                // 302
StatusCode::BadRequest           // 400
StatusCode::Unauthorized         // 401
StatusCode::Forbidden            // 403
StatusCode::NotFound             // 404
StatusCode::MethodNotAllowed     // 405
StatusCode::InternalServerError  // 500
StatusCode::ServiceUnavailable   // 503
// ... all 1xx–5xx codes
```

Helper methods: `StatusCode::ok()`, `StatusCode::not_found()`, etc. Conversion: `StatusCode::from_u16(200)`, `code.as_u16()`.

## JSON Responses

```rust
use loony_server::response::HttpResponse;
use serde::Serialize;

#[derive(Serialize)]
struct User { id: i32, name: String }

async fn get_user() -> impl Responder {
    let user = User { id: 1, name: "Alice".into() };
    HttpResponse::ok().json(user).unwrap()
}
```

Or using the constructor form:

```rust
async fn get_user() -> impl Responder {
    HttpResponse::with_json(&User { id: 1, name: "Alice".into() }).unwrap()
}
```

Both set `Content-Type: application/json` and a correct `Content-Length`.

## Error Responses

```rust
async fn handler(Path(id): Path<i32>) -> impl Responder {
    if id <= 0 {
        return (StatusCode::BadRequest, "id must be positive").respond().await;
    }
    // ...
}
```

Or using `HttpResponse` directly:

```rust
HttpResponse::not_found()
    .body("user not found")
    .build()
```
