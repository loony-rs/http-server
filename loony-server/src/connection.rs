use std::{
    io::{self, Read, Write, ErrorKind},
    net::{TcpStream, Shutdown, SocketAddr},
    time::Duration,
};

/// Represents a single TCP connection with buffered I/O.
pub struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Connection {
    /// Create a new `Connection` from a `TcpStream`, configuring sensible defaults.
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        // Configure timeouts and TCP settings
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_nodelay(true)?; // Disable Nagle’s algorithm for latency-sensitive use
        
        Ok(Self {
            stream,
            buffer: vec![0u8; 8192], // 8KB internal buffer
        })
    }

    /// Reads exactly `buf.len()` bytes or until EOF.
    pub fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut total_read = 0;
        while total_read < buf.len() {
            match self.stream.read(&mut buf[total_read..]) {
                Ok(0) => {
                    // If we read 0 and nothing read so far, treat as EOF
                    if total_read == 0 {
                        return Ok(0);
                    } else {
                        break;
                    }
                }
                Ok(n) => total_read += n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(total_read)
    }

    /// Reads data until a specified delimiter byte is encountered.
    pub fn read_until(&mut self, delimiter: u8) -> io::Result<Vec<u8>> {
        let mut result = Vec::new();
        loop {
            let bytes_read = self.stream.read(&mut self.buffer)?;
            if bytes_read == 0 {
                // EOF reached
                break;
            }

            for &b in &self.buffer[..bytes_read] {
                result.push(b);
                if b == delimiter {
                    return Ok(result);
                }
            }
        }
        Ok(result)
    }

    /// Reads all available data until EOF.
    pub fn read_all(&mut self) -> io::Result<Vec<u8>> {
        let mut result = Vec::new();
        loop {
            match self.stream.read(&mut self.buffer) {
                Ok(0) => break, // EOF
                Ok(n) => result.extend_from_slice(&self.buffer[..n]),
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // For non-blocking sockets: return what’s read so far
                    break;
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(result)
    }

    /// Writes all bytes from `data` to the stream.
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let mut total_written = 0;
        while total_written < data.len() {
            match self.stream.write(&data[total_written..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        ErrorKind::WriteZero,
                        "failed to write the entire buffer",
                    ));
                }
                Ok(n) => total_written += n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        self.stream.flush()?;
        Ok(())
    }

    pub fn read_http_response(&mut self) -> io::Result<Vec<u8>> {
        let mut result = Vec::new();
        let mut buffer = [0u8; 1024];
        let mut content_length = None;
        
        // First, read headers
        loop {
            let bytes_read = self.stream.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            
            result.extend_from_slice(&buffer[..bytes_read]);
            
            // Check if we've received all headers
            if let Some(headers_end) = find_headers_end(&result) {
                // Parse Content-Length header if present
                content_length = parse_content_length(&result[..headers_end]);
                
                // If no content length or zero, we're done
                if content_length.unwrap_or(0) == 0 {
                    break;
                }
                
                // Read the remaining body
                let body_bytes_needed = content_length.unwrap() - (result.len() - headers_end);
                if body_bytes_needed > 0 {
                    let mut body = vec![0u8; body_bytes_needed];
                    self.read_exact(&mut body)?; // Now safe to use read_exact
                    result.extend_from_slice(&body);
                }
                break;
            }
        }
        
        Ok(result)
    }


    /// Writes a UTF-8 string to the connection.
    pub fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write(s.as_bytes())
    }

    /// Flush any buffered writes.
    pub fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }

    /// Gracefully close the connection.
    pub fn close(mut self) -> io::Result<()> {
        self.flush()?;
        self.stream.shutdown(Shutdown::Both)
    }

    /// Returns the peer’s socket address, useful for logging.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Err(e) = self.stream.shutdown(Shutdown::Both) {
            eprintln!("Warning: failed to shutdown connection: {}", e);
        }
    }
}


fn find_headers_end(data: &[u8]) -> Option<usize> {
    // Look for \r\n\r\n sequence that marks end of HTTP headers
    data.windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|pos| pos + 4)
}

fn parse_content_length(headers: &[u8]) -> Option<usize> {
    let header_str = std::str::from_utf8(headers).ok()?;
    for line in header_str.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            return line.split(':')
                    .nth(1)?
                    .trim()
                    .parse()
                    .ok();
        }
    }
    None
}