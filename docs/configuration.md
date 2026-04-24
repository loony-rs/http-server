# Configuration

## HttpServer Builder

`HttpServer` is configured through a fluent builder before calling `.run()`.

```rust
use loony_server::{HttpServer, ServerConfig, TlsConfig};

HttpServer::new(|| App::new().routes(routes))
    .bind(3000)          // port to listen on
    .workers(4)          // number of worker threads
    .with_config(ServerConfig {
        read_timeout:    Duration::from_secs(30),
        write_timeout:   Duration::from_secs(30),
        max_connections: 1000,
        ..ServerConfig::default()
    })
    .tls(TlsConfig {     // optional — enables HTTPS
        cert_path: "cert.pem".into(),
        key_path:  "key.pem".into(),
    })
    .run()
    .await
    .unwrap();
```

### Builder methods

| Method | Description |
|---|---|
| `.bind(port: i32)` | Port to listen on. Default: `2443` |
| `.workers(n: usize)` | Number of worker threads. Default: logical CPU count |
| `.with_config(ServerConfig)` | Supply a full config struct |
| `.tls(TlsConfig)` | Enable HTTPS (see [TLS](tls.md)) |

## ServerConfig

```rust
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub workers: usize,
    pub max_connections: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub tls: Option<TlsConfig>,
}
```

### Defaults

| Field | Default |
|---|---|
| `port` | `3005` |
| `workers` | `std::thread::available_parallelism()` → logical CPU count, fallback `1` |
| `max_connections` | `1000` (currently informational, not enforced at connection level) |
| `read_timeout` | `30` seconds |
| `write_timeout` | `30` seconds |
| `tls` | `None` (plain HTTP) |

### Timeouts

`read_timeout` controls how long the server waits for the next request on a keep-alive connection. When a client is idle for longer than this, the OS returns a `TimedOut` or `WouldBlock` error, which the server treats as a clean close (no error logged).

`write_timeout` bounds each response write. If a slow client cannot consume the response in this time, the connection is closed.

Both timeouts are set via `TcpStream::set_read_timeout` / `set_write_timeout` on the underlying socket.

### Example: low-latency configuration

```rust
use std::time::Duration;

ServerConfig {
    workers: 8,
    read_timeout: Duration::from_secs(10),   // aggressive keep-alive reaping
    write_timeout: Duration::from_secs(10),
    ..ServerConfig::default()
}
```

### Example: high-throughput file server

```rust
ServerConfig {
    workers: 16,
    read_timeout: Duration::from_secs(60),   // allow slow uploads
    write_timeout: Duration::from_secs(120), // allow slow downloads
    ..ServerConfig::default()
}
```

## Security Limits

These are compile-time constants in `connection.rs`:

| Limit | Value | Description |
|---|---|---|
| `MAX_HEADER_BYTES` | 16 384 bytes (16 KB) | Maximum total size of request headers |
| `MAX_BODY_BYTES` | 104 857 600 bytes (100 MB) | Maximum request body (Content-Length or chunked) |

Requests that exceed these limits receive an `InvalidData` I/O error, which closes the connection. No 4xx response is sent (the connection is dropped before the request is dispatched).

## Worker Model

Each worker is an OS thread running its own `tokio::runtime::Builder::new_current_thread()` runtime inside a `LocalSet`. This means:

- `Rc<T>` and `RefCell<T>` are safe to use on the hot path — no `Arc`/`Mutex` overhead
- Workers do not share heap state; each initialises its own copy of the app (your `App::new()` closure is called once per worker)
- `SO_REUSEPORT` lets all workers bind the same address:port; the kernel load-balances incoming TCP connections across them
- Worker threads are named `loony-worker-0`, `loony-worker-1`, … for easier profiling

## Tracing / Logging

Logging is controlled entirely by the `RUST_LOG` environment variable (via `tracing-subscriber`'s `EnvFilter`):

```bash
RUST_LOG=info cargo run            # show info, warn, error
RUST_LOG=debug cargo run           # show all levels
RUST_LOG=loony_server=trace cargo run   # trace only this crate
RUST_LOG=warn cargo run            # suppress info, show only warn/error
```

The subscriber is initialised once by `HttpServer::run()` via `init_tracing()`. Subsequent calls to `init_tracing()` are no-ops (safe to call yourself before `.run()` if you need a custom subscriber):

```rust
use loony_server::init_tracing;

// Initialise before starting the server to use your own subscriber config.
init_tracing();

HttpServer::new(|| App::new().routes(routes))
    .run()
    .await
    .unwrap();
```
