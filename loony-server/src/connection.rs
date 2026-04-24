use std::{
    io::{self, ErrorKind, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    sync::Arc,
    time::Duration,
};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 100 * 1024 * 1024;

// ---------------------------------------------------------------------------
// RawStream — abstracts plain TCP vs TLS
// ---------------------------------------------------------------------------

enum RawStream {
    Tcp(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ServerConnection, TcpStream>>),
}

impl Read for RawStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            RawStream::Tcp(s) => s.read(buf),
            RawStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for RawStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            RawStream::Tcp(s) => s.write(buf),
            RawStream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            RawStream::Tcp(s) => s.flush(),
            RawStream::Tls(s) => s.flush(),
        }
    }
}

impl RawStream {
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self {
            RawStream::Tcp(s) => s.peer_addr(),
            RawStream::Tls(s) => s.get_ref().peer_addr(),
        }
    }

    fn shutdown(&self) -> io::Result<()> {
        match self {
            RawStream::Tcp(s) => s.shutdown(Shutdown::Both),
            RawStream::Tls(s) => s.get_ref().shutdown(Shutdown::Both),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal transfer-encoding descriptor
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum BodyTransfer {
    ContentLength(usize),
    Chunked,
    None,
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

pub struct Connection {
    stream: RawStream,
    buffer: Vec<u8>,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        read_timeout: Duration,
        write_timeout: Duration,
    ) -> io::Result<Self> {
        stream.set_read_timeout(Some(read_timeout))?;
        stream.set_write_timeout(Some(write_timeout))?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream: RawStream::Tcp(stream),
            buffer: vec![0u8; 8192],
        })
    }

    pub fn new_tls(
        stream: TcpStream,
        tls_config: Arc<rustls::ServerConfig>,
        read_timeout: Duration,
        write_timeout: Duration,
    ) -> io::Result<Self> {
        stream.set_read_timeout(Some(read_timeout))?;
        stream.set_write_timeout(Some(write_timeout))?;
        stream.set_nodelay(true)?;
        let conn = rustls::ServerConnection::new(tls_config)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Self {
            stream: RawStream::Tls(Box::new(rustls::StreamOwned::new(conn, stream))),
            buffer: vec![0u8; 8192],
        })
    }

    // ---------------------------------------------------------------------------
    // HTTP framing: read a complete request (headers + body)
    // ---------------------------------------------------------------------------

    pub fn read_http_response(&mut self) -> io::Result<Vec<u8>> {
        // Phase 1 — read until the blank line that terminates headers.
        let mut raw: Vec<u8> = Vec::new();
        let headers_end = loop {
            let n = self.stream.read(&mut self.buffer)?;
            if n == 0 {
                return Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "connection closed before headers were complete",
                ));
            }
            raw.extend_from_slice(&self.buffer[..n]);
            if raw.len() > MAX_HEADER_BYTES {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("request headers exceed {MAX_HEADER_BYTES} byte limit"),
                ));
            }
            if let Some(pos) = find_headers_end(&raw) {
                break pos;
            }
        };

        // Phase 2 — read the body according to the transfer encoding.
        match detect_body_transfer(&raw[..headers_end]) {
            BodyTransfer::ContentLength(len) => {
                if len > MAX_BODY_BYTES {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!("Content-Length {len} exceeds {MAX_BODY_BYTES} byte limit"),
                    ));
                }
                let already_buffered = raw.len() - headers_end;
                if len > already_buffered {
                    let remaining = len - already_buffered;
                    let old_len = raw.len();
                    raw.resize(old_len + remaining, 0);
                    self.stream.read_exact(&mut raw[old_len..])?;
                }
                raw.truncate(headers_end + len);
                Ok(raw)
            },

            BodyTransfer::Chunked => {
                let partial_body = raw[headers_end..].to_vec();
                let decoded_body = self.decode_chunked_body(partial_body)?;
                raw.truncate(headers_end);
                raw.extend_from_slice(&decoded_body);
                Ok(raw)
            },

            BodyTransfer::None => {
                raw.truncate(headers_end);
                Ok(raw)
            },
        }
    }

    // ---------------------------------------------------------------------------
    // Chunked transfer-encoding decoder
    // ---------------------------------------------------------------------------

    fn decode_chunked_body(&mut self, initial: Vec<u8>) -> io::Result<Vec<u8>> {
        let mut body: Vec<u8> = Vec::new();
        let mut buf = initial;

        loop {
            let line_end = loop {
                if let Some(pos) = find_crlf(&buf) {
                    break pos;
                }
                let n = self.stream.read(&mut self.buffer)?;
                if n == 0 {
                    return Err(io::Error::new(
                        ErrorKind::UnexpectedEof,
                        "unexpected EOF while reading chunk size",
                    ));
                }
                buf.extend_from_slice(&self.buffer[..n]);
            };

            let size_line = std::str::from_utf8(&buf[..line_end])
                .map_err(|_| io::Error::new(ErrorKind::InvalidData, "non-UTF8 chunk size line"))?;
            let hex_part = size_line.split(';').next().unwrap_or("").trim();
            let chunk_size = usize::from_str_radix(hex_part, 16).map_err(|_| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid chunk size: '{hex_part}'"),
                )
            })?;

            buf.drain(..line_end + 2);

            if chunk_size == 0 {
                break;
            }

            let needed = chunk_size + 2;
            while buf.len() < needed {
                let n = self.stream.read(&mut self.buffer)?;
                if n == 0 {
                    return Err(io::Error::new(
                        ErrorKind::UnexpectedEof,
                        "unexpected EOF while reading chunk data",
                    ));
                }
                buf.extend_from_slice(&self.buffer[..n]);
            }

            body.extend_from_slice(&buf[..chunk_size]);
            if body.len() > MAX_BODY_BYTES {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("chunked body exceeds {MAX_BODY_BYTES} byte limit"),
                ));
            }
            buf.drain(..needed);
        }

        Ok(body)
    }

    // ---------------------------------------------------------------------------
    // Low-level I/O helpers
    // ---------------------------------------------------------------------------

    pub fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut total = 0;
        while total < buf.len() {
            match self.stream.read(&mut buf[total..]) {
                Ok(0) => {
                    if total == 0 {
                        return Ok(0);
                    } else {
                        break;
                    }
                },
                Ok(n) => total += n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(total)
    }

    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let mut written = 0;
        while written < data.len() {
            match self.stream.write(&data[written..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        ErrorKind::WriteZero,
                        "failed to write the entire buffer",
                    ));
                },
                Ok(n) => written += n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        self.stream.flush()
    }

    pub fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write(s.as_bytes())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }

    pub fn close(mut self) -> io::Result<()> {
        self.flush()?;
        self.stream.shutdown()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.stream.shutdown();
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn find_headers_end(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

fn find_crlf(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w == b"\r\n")
}

fn detect_body_transfer(headers: &[u8]) -> BodyTransfer {
    let text = match std::str::from_utf8(headers) {
        Ok(s) => s,
        Err(_) => return BodyTransfer::None,
    };

    let mut content_length: Option<usize> = None;

    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            return BodyTransfer::Chunked;
        }
        if lower.starts_with("content-length:") {
            content_length = line
                .splitn(2, ':')
                .nth(1)
                .and_then(|v| v.trim().parse().ok());
        }
    }

    match content_length {
        Some(0) | None => BodyTransfer::None,
        Some(n) => BodyTransfer::ContentLength(n),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_end_found() {
        let data = b"GET / HTTP/1.1\r\nHost: x\r\n\r\nbody";
        assert_eq!(find_headers_end(data), Some(27));
    }

    #[test]
    fn headers_end_not_found() {
        let data = b"GET / HTTP/1.1\r\nHost: x\r\n";
        assert_eq!(find_headers_end(data), None);
    }

    #[test]
    fn detect_content_length() {
        let headers = b"POST / HTTP/1.1\r\nContent-Length: 13\r\n\r\n";
        match detect_body_transfer(headers) {
            BodyTransfer::ContentLength(13) => {},
            other => panic!("expected ContentLength(13), got {other:?}"),
        }
    }

    #[test]
    fn detect_chunked() {
        let headers = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(matches!(detect_body_transfer(headers), BodyTransfer::Chunked));
    }

    #[test]
    fn detect_chunked_overrides_content_length() {
        let headers =
            b"POST / HTTP/1.1\r\nContent-Length: 99\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(matches!(detect_body_transfer(headers), BodyTransfer::Chunked));
    }

    #[test]
    fn detect_none_for_get() {
        let headers = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert!(matches!(detect_body_transfer(headers), BodyTransfer::None));
    }

    #[test]
    fn detect_none_for_zero_content_length() {
        let headers = b"POST / HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
        assert!(matches!(detect_body_transfer(headers), BodyTransfer::None));
    }

    #[test]
    fn request_parse_extracts_body() {
        use crate::request::HttpRequest;
        let raw = b"POST /echo HTTP/1.1\r\nContent-Length: 13\r\n\r\nHello, world!";
        let mut req = HttpRequest::new();
        req.parse(raw).unwrap();
        let body = req.body.expect("body should be Some");
        assert_eq!(&body[..], b"Hello, world!");
    }

    #[test]
    fn request_parse_no_body_for_get() {
        use crate::request::HttpRequest;
        let raw = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut req = HttpRequest::new();
        req.parse(raw).unwrap();
        assert!(req.body.is_none());
    }

    #[test]
    fn find_crlf_found() {
        assert_eq!(find_crlf(b"5\r\nhello\r\n"), Some(1));
    }

    #[test]
    fn find_crlf_not_found() {
        assert_eq!(find_crlf(b"hello"), None);
    }
}
