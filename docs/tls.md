# TLS / HTTPS

loony-http-server supports HTTPS via [`rustls`](https://github.com/rustls/rustls) — a pure-Rust TLS implementation with no dependency on OpenSSL or any system TLS library.

## Quick Setup

```rust
use loony_server::{App, HttpServer, TlsConfig, route, router::Router};

fn routes() -> Router {
    Router::new().route(route::get("/").to(hello))
}

#[tokio::main]
async fn main() {
    HttpServer::new(|| App::new().routes(routes))
        .tls(TlsConfig {
            cert_path: "certs/cert.pem".into(),
            key_path:  "certs/key.pem".into(),
        })
        .bind(443)
        .run()
        .await
        .unwrap();
}
```

## Generating a Self-Signed Certificate (Development)

Using `openssl`:

```bash
mkdir -p certs
openssl req -x509 -newkey rsa:4096 -keyout certs/key.pem \
    -out certs/cert.pem -days 365 -nodes \
    -subj "/CN=localhost"
```

Using `mkcert` (trusted in your local browser):

```bash
mkcert -install
mkcert -key-file certs/key.pem -cert-file certs/cert.pem localhost 127.0.0.1
```

Test the server:

```bash
curl -k https://localhost:443/     # -k skips cert verification for self-signed
curl --cacert certs/cert.pem https://localhost:443/   # verify with the cert
```

## TlsConfig Fields

```rust
pub struct TlsConfig {
    pub cert_path: String,  // path to PEM certificate chain
    pub key_path: String,   // path to PEM private key
}
```

Both files must be PEM-encoded. The certificate file may contain a full chain (leaf + intermediates). The key file must contain exactly one private key in PKCS#8 or SEC1 format.

## load_tls_config

You can build a `rustls::ServerConfig` directly if you need programmatic control (e.g., loading certs from a secrets manager):

```rust
use loony_server::load_tls_config;
use std::sync::Arc;

let tls = load_tls_config("cert.pem", "key.pem")?;
// tls: Arc<rustls::ServerConfig>
```

The function:
1. Opens and parses the PEM certificate file with `rustls-pemfile`
2. Opens and parses the PEM private key file
3. Builds a `rustls::ServerConfig` with no client authentication
4. Returns `Arc<rustls::ServerConfig>`

Errors are returned as `std::io::Error` with descriptive messages.

## How It Works Internally

When TLS is configured, each worker thread passes the `Arc<rustls::ServerConfig>` into `Connection::new_tls()`. This wraps the raw `TcpStream` in a `rustls::StreamOwned<ServerConnection, TcpStream>`, which implements `Read + Write`. The TLS handshake is performed on the first read (driven by `rustls` internally). All subsequent I/O — including HTTP framing, keep-alive loops, and timeouts — is identical to the plain-TCP path.

The `Arc<rustls::ServerConfig>` is cloned once per worker (at startup) and once per accepted connection. The per-connection `ServerConnection` is stack-allocated inside the `StreamOwned`.

## Production Certificates

For production, obtain a certificate from a CA such as [Let's Encrypt](https://letsencrypt.org/):

```bash
# Using certbot (standalone mode — requires port 80 to be free)
certbot certonly --standalone -d yourdomain.com

# Certificates are written to:
# /etc/letsencrypt/live/yourdomain.com/fullchain.pem  ← cert_path
# /etc/letsencrypt/live/yourdomain.com/privkey.pem    ← key_path
```

```rust
HttpServer::new(|| App::new().routes(routes))
    .tls(TlsConfig {
        cert_path: "/etc/letsencrypt/live/yourdomain.com/fullchain.pem".into(),
        key_path:  "/etc/letsencrypt/live/yourdomain.com/privkey.pem".into(),
    })
    .bind(443)
    .run()
    .await
    .unwrap();
```

## Mixing HTTP and HTTPS

To serve both HTTP and HTTPS simultaneously, start two server instances in separate tasks:

```rust
#[tokio::main]
async fn main() {
    let http = tokio::spawn(async {
        HttpServer::new(|| App::new().routes(routes))
            .bind(80)
            .run()
            .await
    });

    let https = tokio::spawn(async {
        HttpServer::new(|| App::new().routes(routes))
            .tls(TlsConfig {
                cert_path: "cert.pem".into(),
                key_path:  "key.pem".into(),
            })
            .bind(443)
            .run()
            .await
    });

    let _ = tokio::join!(http, https);
}
```

## Security Notes

- rustls always enforces TLS 1.2+ — TLS 1.0 and 1.1 are not supported
- Client certificate authentication is not enabled (no-client-auth mode)
- The private key file should be readable only by the server process (`chmod 600`)
- Rotate certificates before expiry; the server must be restarted to pick up new certs
