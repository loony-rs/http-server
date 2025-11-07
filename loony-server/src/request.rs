use crate::extensions::Extensions;
use std::rc::Rc;
use std::slice::Iter;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Header<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}

pub const EMPTY_HEADER: Header<'static> = Header { name: "", value: b"" };
#[derive(Debug)]
pub struct Request {
    pub method: Option<String>,
    pub uri: Option<String>,
    pub version: Option<String>,
}

impl Request {
    pub fn new(_: &[Header; 16]) -> Self {
        Self {
            method: None,
            uri: None,
            version: None,
        }
    }

    pub fn parse(&mut self, buffer_ref: &[u8]) {
        let mut iter_buffer = buffer_ref.iter();

        let mut index:usize = 0;

        let method = head(&mut iter_buffer, &mut index);
        let uri = head(&mut iter_buffer, &mut index);
        let version = get_version(&mut iter_buffer, &mut index);

        if let Some((mh, mt)) = method {
            let a = String::from_utf8_lossy(&buffer_ref[mh..mt]).to_string();
            self.method = Some(a);
        }
        if let Some((uh, ut)) = uri {
            let a = String::from_utf8_lossy(&buffer_ref[uh..ut]).to_string();
            self.uri = Some(a);
        }
        if let Some((vh, vt)) = version {
            let a = String::from_utf8_lossy(&buffer_ref[vh..vt]).to_string();
            self.version = Some(a);
        }
    }
}


fn head(buffer: &mut Iter<u8>, index: &mut usize) -> Option<(usize, usize)> {
    let mut count: usize = 0;
    let start = *index;

    loop {
        match buffer.next() {
            Some(b) => {
                if *b == b' ' {
                    let rd = (start, start + count);
                    *index = start + count + 1;
                    return Some(rd);
                }
                count += 1;
            }
            None => {
                return None;
            }
        }
    }
}

fn get_version(buffer: &mut Iter<u8>, index: &mut usize) -> Option<(usize, usize)> {
    let mut count: usize = 0;
    let start = *index;

    loop {
        match buffer.next() {
            Some(b) => {
                if *b == b'\n' {
                    let rd = (start, start + count);
                    *index = start + count + 1;
                    return Some(rd);
                }
                count += 1;
            }
            None => {
                return None;
            }
        }
    }
}

#[derive(Clone)]
pub struct HttpRequest {
    pub url: String,
    pub params: Option<Vec<String>>,
    pub extensions: Rc<Extensions>
}