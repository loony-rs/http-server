# loony-http-server

A lightweight, async HTTP/1.1 server written from scratch in Rust. Built on Tokio with a multi-threaded worker model, it uses per-thread `!Send` types (`Rc`, `RefCell`) for zero-overhead routing — no `Arc` or `Mutex` on the hot path.

## Features

- Multi-threaded worker pool with `SO_REUSEPORT` — the kernel distributes connections across workers
- Radix-tree router with typed path parameters (`/user/:id`, `/user/:id/:name`)
- HTTP/1.1 keep-alive with configurable per-connection read/write timeouts
- Middleware system — chainable, composable; built-in `Logger` and `Cors`
- Typed extractors: `Data<T>` (shared state), `Path<T>` (URL params), `Json<T>` (request body)
- `Responder` trait — return `String`, `HttpResponse`, `(StatusCode, body)`, `Html<T>`, and more
- Optional TLS via `rustls` — pure Rust, no OpenSSL dependency
- Security hardening: 16 KB header limit, 100 MB body limit, header injection prevention, enforced `Content-Length`
- Structured logging via the `tracing` crate (`RUST_LOG` env var)

## Quick Start

```toml
# Cargo.toml
[dependencies]
loony-server = { path = "./loony-server" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
use loony_server::{App, HttpServer, responder::Responder, route, router::Router};

async fn hello() -> impl Responder {
    "Hello, world!"
}

fn routes() -> Router {
    Router::new().route(route::get("/").to(hello))
}

#[tokio::main]
async fn main() {
    HttpServer::new(|| App::new().routes(routes))
        .bind(3000)
        .run()
        .await
        .unwrap();
}
```

```
$ RUST_LOG=info cargo run
2025-04-24T10:00:00Z  INFO loony_server::server: request peer="127.0.0.1:54321" method="GET" path="/" status="200" latency_ms=1
```

## Documentation

| Topic | File |
|---|---|
| Getting Started | [docs/getting-started.md](docs/getting-started.md) |
| Routing | [docs/routing.md](docs/routing.md) |
| Extractors | [docs/extractors.md](docs/extractors.md) |
| Responses & Responder | [docs/responses.md](docs/responses.md) |
| Middleware | [docs/middleware.md](docs/middleware.md) |
| TLS / HTTPS | [docs/tls.md](docs/tls.md) |
| Configuration | [docs/configuration.md](docs/configuration.md) |
| Architecture | [docs/architecture.md](docs/architecture.md) |

## Project Layout

```
loony-http-server/
├── src/                        # Example application (uses the library)
│   ├── main.rs
│   └── controller.rs
├── loony-server/               # Core HTTP server library
│   └── src/
│       ├── app.rs              # App builder — routes, data, middleware
│       ├── connection.rs       # TCP/TLS I/O framing, size limits
│       ├── extract.rs          # Typed extractors (Data, Path, Json)
│       ├── handler.rs          # Handler trait bridge
│       ├── middleware.rs       # Middleware trait + Logger + Cors
│       ├── request.rs          # HttpRequest parser (httparse)
│       ├── responder.rs        # Responder trait implementations
│       ├── response.rs         # HttpResponse builder, StatusCode
│       ├── route.rs            # Route definition (GET, POST, …)
│       ├── router.rs           # Router builder
│       ├── scope.rs            # Route scope grouping
│       └── server.rs           # HttpServer, ServerConfig, TLS
├── loony-router/               # Radix-tree router (standalone crate)
└── loony-service/              # Service / ServiceFactory trait definitions
```

## Example: Full Route Set

```rust
use loony_server::{
    App, HttpServer, Json, Path,
    extract::Data,
    responder::Responder,
    response::{HttpResponse, StatusCode},
    route, router::Router,
    middleware::{Logger, Cors},
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct AppState { db_url: String }

#[derive(Deserialize)]
struct CreateUser { name: String }

#[derive(Serialize)]
struct User { id: i32, name: String }

async fn get_user(Path(id): Path<i32>) -> impl Responder {
    HttpResponse::ok().json(User { id, name: "Alice".into() }).unwrap()
}

async fn create_user(
    Data(state): Data<AppState>,
    Json(body): Json<CreateUser>,
) -> impl Responder {
    println!("db: {}, creating: {}", state.db_url, body.name);
    HttpResponse::ok().body("created")
}

fn routes() -> Router {
    Router::new().service(
        route::scope("/api")
            .route(route::get("/user/:id").to(get_user))
            .route(route::post("/user").to(create_user)),
    )
}

#[tokio::main]
async fn main() {
    let state = AppState { db_url: "postgres://localhost/mydb".into() };
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(Logger)
            .wrap(Cors::new())
            .routes(routes)
    })
    .workers(4)
    .bind(3000)
    .run()
    .await
    .unwrap();
}
```

## Security Defaults

| Limit | Default |
|---|---|
| Max request headers | 16 KB |
| Max request body | 100 MB |
| Read timeout | 30 s |
| Write timeout | 30 s |
| Header injection | CR/LF stripped from response header values |
| Content-Length | Auto-corrected in `HttpResponse::build()` |

## License

MIT OR Apache-2.0
