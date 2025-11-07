use httparse::{Request, Status};

pub const EMPTY_HEADER: Header<'static> = Header { name: "", value: b"" };

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
    pub params: Option<String>
}

impl HttpRequest {
    pub fn new() -> Self {
        Self {
            method: None,
            uri: None,
            version: None,
            headers: Vec::new(),
            params: None
        }
    }

    pub fn parse(&mut self, buffer: &[u8]) -> Result<usize, &'static str> {
        // Create a headers array with a fixed size (common practice is 16-64)
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = Request::new(&mut headers);
        
        // Parse the request
        match req.parse(buffer) {
            Ok(Status::Complete(parsed_len)) => {
                // Store method
                if let Some(method) = req.method {
                    self.method = Some(method.to_string());
                }
                
                // Store URI
                if let Some(path) = req.path {
                    self.uri = Some(path.to_string());
                }
                
                // Store version
                if let Some(version) = req.version {
                    self.version = Some(version);
                }
                
                // Store headers
                self.headers.clear();
                for header in req.headers.iter() {
                    let name = header.name.to_string();
                    let value = String::from_utf8_lossy(header.value).to_string();
                    self.headers.push((name, value));
                }
                
                Ok(parsed_len)
            }
            Ok(Status::Partial) => {
                Err("Incomplete HTTP request")
            }
            Err(e) => {
                eprintln!("Parse error: {:?}", e);
                Err("Failed to parse HTTP request")
            }
        }
    }
}

// Optional: Implement Default trait
impl Default for HttpRequest {
    fn default() -> Self {
        Self::new()
    }
}
