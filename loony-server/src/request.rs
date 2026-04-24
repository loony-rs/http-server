use bytes::Bytes;
use httparse::{Request, Status};
use std::rc::Rc;

pub const EMPTY_HEADER: Header<'static> = Header {
    name: "",
    value: b"",
};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Header<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Option<String>,
    pub uri: Option<String>,
    pub version: Option<u8>,
    pub headers: Vec<(String, String)>,
    pub params: Rc<Vec<String>>,
    /// Raw request body bytes.  `None` for requests without a body
    /// (GET, DELETE, HEAD, etc.) or when `Content-Length` is 0.
    pub body: Option<Bytes>,
}

impl HttpRequest {
    pub fn new() -> Self {
        Self {
            method: None,
            uri: None,
            version: None,
            headers: Vec::new(),
            params: Rc::new(Vec::new()),
            body: None,
        }
    }

    /// Parse a raw HTTP request buffer.
    ///
    /// `buffer` must contain the full request: request line, headers, blank
    /// line, and (for POST/PUT) the body.  Returns the number of bytes
    /// consumed by the headers section on success.
    ///
    /// Body bytes (`buffer[returned_offset..]`) are stored in `self.body`.
    pub fn parse(&mut self, buffer: &[u8]) -> Result<usize, &'static str> {
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut req = Request::new(&mut headers);

        match req.parse(buffer) {
            Ok(Status::Complete(headers_len)) => {
                if let Some(method) = req.method {
                    self.method = Some(method.to_string());
                }
                if let Some(path) = req.path {
                    let (_, params) = parse_uri(path);
                    self.uri = Some(path.to_string());
                    self.params = Rc::new(params);
                }
                if let Some(version) = req.version {
                    self.version = Some(version);
                }

                self.headers.clear();
                for header in req.headers.iter() {
                    if header.name.is_empty() {
                        break;
                    }
                    let name = header.name.to_string();
                    // HTTP headers are Latin-1; lossy conversion preserves ASCII
                    // and replaces rare non-ASCII bytes with the replacement char.
                    let value = String::from_utf8_lossy(header.value).into_owned();
                    self.headers.push((name, value));
                }

                // Body: everything after the header section.
                let body_bytes = &buffer[headers_len..];
                if !body_bytes.is_empty() {
                    self.body = Some(Bytes::copy_from_slice(body_bytes));
                }

                Ok(headers_len)
            },
            Ok(Status::Partial) => Err("incomplete HTTP request"),
            Err(_) => Err("malformed HTTP request"),
        }
    }
}

impl Default for HttpRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a URI into its path and query-parameter strings.
fn parse_uri(uri: &str) -> (String, Vec<String>) {
    let mut parts = uri.splitn(2, '?');
    let path = parts.next().unwrap_or("").to_string();
    let params = parts
        .next()
        .map(|q| q.split('&').map(String::from).collect())
        .unwrap_or_default();
    (path, params)
}
