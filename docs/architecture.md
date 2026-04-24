# Architecture

## Overview

loony-http-server is composed of three crates:

| Crate | Role |
|---|---|
| `loony-service` | Core `Service` and `ServiceFactory` traits (modelled on tower) |
| `loony-router` | Standalone radix-tree router |
| `loony-server` | HTTP server — framing, routing, extractors, middleware, TLS |

## Request Lifecycle

```
TCP accept (per worker)
  │
  ▼
Connection::read_http_response()
  │  ├─ reads until \r\n\r\n (16 KB header limit)
  │  ├─ reads body: Content-Length (100 MB limit) or chunked decode
  │  └─ returns raw bytes
  │
  ▼
Run::parse_request()
  │  └─ httparse → HttpRequest { method, uri, version, headers, body }
  │
  ▼
Run::dispatch()
  │  ├─ strip query string from URI
  │  ├─ RadixRouter::find_route() → (service_index, Vec<String> params)
  │  └─ call Run::call_service()
  │
  ▼
build_middleware_chain() [if middlewares registered]
  │  └─ Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>>
  │     folded: M0(M1(M2(handler)))
  │
  ▼
ExtractService::call()
  │  ├─ T::from_request(&req) — extracts Data/Path/Json/etc.
  │  └─ handler(extracted_args).await → impl Responder
  │
  ▼
Responder::respond() → ServiceResponse(String)
  │
  ▼
inject_connection_header()   (adds Connection: keep-alive / close)
  │
  ▼
Connection::write_str()      (writes serialised response to socket)
  │
  ▼
keep-alive loop or close
```

## Worker Model

```
main thread
  │
  ├─ HttpServer::run()
  │    ├─ build TLS config (if configured)
  │    ├─ spawn loony-worker-0
  │    ├─ spawn loony-worker-1
  │    └─ ...
  │
  └─ pending::<()>().await  (parks main thread forever)

loony-worker-N  (OS thread)
  │
  └─ current_thread Tokio runtime
       └─ LocalSet
            └─ ServeHttpService::run()
                 └─ Run::run()  (accept loop)
                      └─ handle_connection() per client
```

Each worker independently:
1. Calls the `App::new()` closure to build its own `AppFactory`
2. Calls `new_service(())` to get an `AppHttpService` with its own `AllRouteServices` and `Extensions`
3. Binds the same port via `SO_REUSEPORT`
4. Runs a `current_thread` accept loop

There is **no shared mutable state** between workers. `Rc<T>` is safe on the hot path.

## Key Types

### Service / ServiceFactory (loony-service)

```rust
trait Service {
    type Request; type Response; type Error; type Future;
    fn call(&mut self, req: Self::Request) -> Self::Future;
}

trait ServiceFactory {
    type Service: Service;
    fn new_service(&self, cfg: Self::Config) -> Self::Future;
}
```

These are the core composition primitives, analogous to tower's `Service`. Routes, extractors, and handlers all compose as `ServiceFactory` chains.

### Handler composition

```
Factory<F, T, R>           (wraps an async fn)
  └─ Extract<T, S>         (runs T::from_request before calling S)
       └─ Handler<F, T, R> (calls the fn with extracted T)
```

The entire chain is a `ServiceFactory`. `new_service()` produces the live `Service`. `call()` drives the future.

### ExtractResponse — zero-clone extraction

`ExtractResponse` stores `req: Option<ServiceRequest>`. On first poll, `from_request` borrows the inner `ServiceRequest` to extract `T`. When ready, `take()` moves the request into the handler call without cloning. This eliminates two `clone()` calls that existed in the naive implementation.

### Radix Router (loony-router)

The router is a tree of `RadixNode`s:

```rust
struct RadixNode {
    static_children: Vec<(String, Box<RadixNode>)>,  // linear scan — cache-friendly
    param_child: Option<(String, Box<RadixNode>)>,    // at most one per node
    service_index: Option<usize>,
}
```

`static_children` is a `Vec` rather than a `HashMap` because route trees typically have few children per node. Linear scan on a small `Vec` is faster than hashing due to CPU cache locality.

`find_route` returns `(service_index, Vec<String>)` — the captured parameter values in path order. No allocation for static-only paths (the `Vec` is pushed-to and popped-from in place during backtracking).

### Connection framing

```
Connection {
    stream: RawStream { Tcp(TcpStream) | Tls(Box<StreamOwned<ServerConnection, TcpStream>>) },
    buffer: Vec<u8>,   // 8 KB read buffer, reused across requests
}
```

`RawStream` implements `Read + Write` for both variants. The TLS handshake is driven lazily by `rustls::StreamOwned` on the first `read()`. Timeouts apply equally to both variants (set via `set_read_timeout` / `set_write_timeout` on the inner `TcpStream` before wrapping).

### Middleware chain

```rust
type Chain = Rc<dyn Fn(ServiceRequest) -> BoxFuture<ServiceResponse>>;

fn build_middleware_chain(middlewares: &[Rc<dyn Middleware>], handler: ...) -> Chain {
    middlewares.iter().rev().fold(base_handler, |inner, mw| {
        Rc::new(move |req| mw.handle(req, Next::new(Rc::clone(&inner))))
    })
}
```

All closures and futures are `!Send` (use `Rc`, not `Arc`). This is safe because each worker thread owns its own chain exclusively.

## Module Map

```
loony-server/src/
├── app.rs          App builder (extensions, services, middlewares)
├── app_service.rs  AppFactory → AppHttpService; registers all routes
├── connection.rs   RawStream enum, TCP/TLS framing, size limits
├── error.rs        ServerError, ParseError, HandlerError hierarchy
├── extensions.rs   TypeMap for per-app shared state
├── extract.rs      FromRequest trait + Data/Path/Json/String impls
│                   Extract<T,S> ServiceFactory, ExtractResponse future
├── handler.rs      Factory<F,T,R>: bridges async fn → Service
├── lib.rs          Public re-exports
├── middleware.rs   Middleware trait, Next, Logger, Cors, inject_headers
├── request.rs      HttpRequest: httparse wrapper + query param split
├── resource.rs     Resource: collects multiple Routes for one path
├── responder.rs    Responder trait + String/HttpResponse/Html/… impls
├── response.rs     HttpResponse builder, StatusCode enum
├── route.rs        Route { path, method, service }; get()/post() fns
├── router.rs       Router builder, AllRouteServices (radix tree wrapper)
├── scope.rs        Scope: path prefix + child routes
└── server.rs       HttpServer, Run, ServeHttpService, TlsConfig, tracing
```

## Design Decisions

**Per-thread `!Send` types** — Using `Rc<RefCell<T>>` on the hot path avoids atomic operations entirely. The tradeoff is that each worker thread must independently initialise its own router and state, but this is a one-time cost at startup.

**`SO_REUSEPORT`** — Multiple sockets bound to the same address are supported on Linux 3.9+ and macOS. The kernel performs connection-level load balancing. This is simpler and more performant than a single accept loop with work-stealing.

**Radix tree over `HashMap`** — O(path depth) routing with cache-friendly traversal. Path depth is typically 2–5, making this faster in practice than a hash of the full path string.

**Synchronous socket I/O inside `spawn_local`** — `Connection` uses blocking `std::net::TcpStream` with `read_timeout`/`write_timeout`. The socket is wrapped in `tokio::task::spawn_local`, which runs on the current-thread runtime. Read/write operations block the Tokio thread only for their duration, which is bounded by the configured timeouts. This avoids `tokio::net::TcpStream`'s more complex async path for the inner I/O loop.

**rustls over native-tls** — Zero system dependencies; consistent behaviour across Linux, macOS, and Windows. TLS 1.2+ only (no legacy protocol negotiation). Certificate loading via `rustls-pemfile`.
