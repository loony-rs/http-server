# Getting Started

## Prerequisites

- Rust 1.85+ (edition 2024)
- Cargo

## Installation

This is a workspace project. Add `loony-server` as a path dependency:

```toml
[dependencies]
loony-server = { path = "./loony-server" }
tokio = { version = "1", features = ["full"] }
```

## Hello World

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

Run it:

```bash
RUST_LOG=info cargo run
```

Test it:

```bash
curl http://localhost:3000/
# Hello, world!
```

## Enabling Logging

The server uses the `tracing` crate. Set `RUST_LOG` before starting:

```bash
RUST_LOG=info cargo run          # info and above
RUST_LOG=debug cargo run         # all debug output
RUST_LOG=loony_server=trace cargo run  # trace only this crate
```

Log format:

```
2025-04-24T10:00:00Z  INFO loony_server::server: request peer="127.0.0.1:54321" method="GET" path="/" status="200" latency_ms=1
```

You can also call `init_tracing()` manually before starting the server if you need custom subscriber configuration:

```rust
use loony_server::init_tracing;

#[tokio::main]
async fn main() {
    init_tracing(); // respects RUST_LOG; safe to call multiple times
    // ...
}
```

## Workers

By default the server spawns one worker thread per logical CPU. Override with `.workers()`:

```rust
HttpServer::new(|| App::new().routes(routes))
    .workers(4)
    .bind(3000)
    .run()
    .await
    .unwrap();
```

Each worker runs its own Tokio `current_thread` runtime. All workers bind the same port via `SO_REUSEPORT` — the OS kernel distributes incoming connections across them.

## Next Steps

- [Routing](routing.md) — path parameters, scopes, HTTP methods
- [Extractors](extractors.md) — typed access to request data
- [Responses](responses.md) — building and returning responses
- [Middleware](middleware.md) — Logger, CORS, custom middleware
- [TLS](tls.md) — HTTPS setup with rustls
- [Configuration](configuration.md) — full `ServerConfig` reference
