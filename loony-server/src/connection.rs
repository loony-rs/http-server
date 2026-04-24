use std::{
    io::{self, ErrorKind, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    time::Duration,
};

/// Represents a single TCP connection with buffered I/O.
pub struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Internal transfer-encoding descriptor
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum BodyTransfer {
    /// Body size is known; value is the exact byte count.
    ContentLength(usize),
    /// Body is sent as a sequence of sized chunks.
    Chunked,
    /// No body expected (GET, DELETE, HEAD, etc.).
    None,
}

impl Connection {
    /// Create a new `Connection` from a `TcpStream`, configuring sensible defaults.
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            buffer: vec![0u8; 8192],
        })
    }

    // ---------------------------------------------------------------------------
    // HTTP framing: read a complete request (headers + body)
    // ---------------------------------------------------------------------------

    /// Read one complete HTTP request from the socket.
    ///
    /// Returns raw bytes containing the request line, all headers, the blank
    /// line, and the decoded body.  For chunked bodies the chunk framing is
    /// stripped; callers always see a contiguous body byte range starting
    /// immediately after the final `\r\n\r\n`.
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
            if let Some(pos) = find_headers_end(&raw) {
                break pos; // pos is the index of the first body byte
            }
        };

        // Phase 2 — read the body according to the transfer encoding.
        match detect_body_transfer(&raw[..headers_end]) {
            BodyTransfer::ContentLength(len) => {
                let already_buffered = raw.len() - headers_end;

                if len > already_buffered {
                    // Need more bytes from the socket.
                    let remaining = len - already_buffered;
                    let old_len = raw.len();
                    raw.resize(old_len + remaining, 0);
                    self.stream.read_exact(&mut raw[old_len..])?;
                }

                // Trim any bytes read beyond Content-Length (guards against
                // requests where one syscall pulled in the next request's
                // bytes — important for keep-alive, Step 6).
                raw.truncate(headers_end + len);
                Ok(raw)
            },

            BodyTransfer::Chunked => {
                // Bytes already past the header boundary may be partial chunk data.
                let partial_body = raw[headers_end..].to_vec();
                let decoded_body = self.decode_chunked_body(partial_body)?;

                // Replace raw with: headers section + decoded body.
                raw.truncate(headers_end);
                raw.extend_from_slice(&decoded_body);
                Ok(raw)
            },

            BodyTransfer::None => {
                // Discard anything read beyond the headers (shouldn't exist
                // for GET/DELETE, but be defensive).
                raw.truncate(headers_end);
                Ok(raw)
            },
        }
    }

    // ---------------------------------------------------------------------------
    // Chunked transfer-encoding decoder
    // ---------------------------------------------------------------------------

    /// Read and decode a chunked body.
    ///
    /// `initial` contains bytes already pulled from the socket after the
    /// header boundary.  The method reads additional data as needed.
    fn decode_chunked_body(&mut self, initial: Vec<u8>) -> io::Result<Vec<u8>> {
        let mut body: Vec<u8> = Vec::new();
        let mut buf = initial;

        loop {
            // Ensure we have at least a chunk-size line (ends with \r\n).
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

            // Parse the chunk size (hex), ignoring optional chunk-extensions.
            let size_line = std::str::from_utf8(&buf[..line_end])
                .map_err(|_| io::Error::new(ErrorKind::InvalidData, "non-UTF8 chunk size line"))?;
            let hex_part = size_line.split(';').next().unwrap_or("").trim();
            let chunk_size = usize::from_str_radix(hex_part, 16).map_err(|_| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid chunk size: '{hex_part}'"),
                )
            })?;

            buf.drain(..line_end + 2); // consume size line + CRLF

            if chunk_size == 0 {
                // Terminal chunk — trailers (if any) and the final CRLF are
                // discarded.  We don't support trailers in Step 3.
                break;
            }

            // Read exactly chunk_size bytes + 2 (the trailing CRLF).
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
            buf.drain(..needed); // consume data + trailing CRLF
        }

        Ok(body)
    }

    // ---------------------------------------------------------------------------
    // Low-level I/O helpers
    // ---------------------------------------------------------------------------

    /// Read exactly `buf.len()` bytes, blocking until all are available.
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

    /// Write all bytes from `data` to the socket.
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

    /// Write a UTF-8 string to the connection.
    pub fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write(s.as_bytes())
    }

    /// Flush any buffered writes.
    pub fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }

    /// Gracefully shut down the connection.
    pub fn close(mut self) -> io::Result<()> {
        self.flush()?;
        self.stream.shutdown(Shutdown::Both)
    }

    /// Returns the remote socket address, useful for logging.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Find the index of the first byte *after* the `\r\n\r\n` header terminator.
fn find_headers_end(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Find the position of the first `\r\n` in `data`.
fn find_crlf(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w == b"\r\n")
}

/// Determine how the request body is framed.
///
/// Reads the raw header bytes (up to the blank line) and looks for
/// `Transfer-Encoding` and `Content-Length`.  `Transfer-Encoding: chunked`
/// takes priority over `Content-Length` per RFC 7230 §3.3.3 rule 3.
fn detect_body_transfer(headers: &[u8]) -> BodyTransfer {
    let text = match std::str::from_utf8(headers) {
        Ok(s) => s,
        Err(_) => return BodyTransfer::None,
    };

    let mut content_length: Option<usize> = None;

    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            // Chunked takes priority; stop scanning.
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

    // --- find_headers_end ---

    #[test]
    fn headers_end_found() {
        // "GET / HTTP/1.1\r\nHost: x\r\n\r\n" → 27 bytes before "body"
        let data = b"GET / HTTP/1.1\r\nHost: x\r\n\r\nbody";
        assert_eq!(find_headers_end(data), Some(27));
    }

    #[test]
    fn headers_end_not_found() {
        let data = b"GET / HTTP/1.1\r\nHost: x\r\n";
        assert_eq!(find_headers_end(data), None);
    }

    // --- detect_body_transfer ---

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
        assert!(matches!(
            detect_body_transfer(headers),
            BodyTransfer::Chunked
        ));
    }

    #[test]
    fn detect_chunked_overrides_content_length() {
        let headers =
            b"POST / HTTP/1.1\r\nContent-Length: 99\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(matches!(
            detect_body_transfer(headers),
            BodyTransfer::Chunked
        ));
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

    // --- HttpRequest body extraction ---

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

    // --- find_crlf ---

    #[test]
    fn find_crlf_found() {
        assert_eq!(find_crlf(b"5\r\nhello\r\n"), Some(1));
    }

    #[test]
    fn find_crlf_not_found() {
        assert_eq!(find_crlf(b"hello"), None);
    }
}
