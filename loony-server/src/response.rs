// use crate::{
//     service::ServiceRequest,
//     resource::ResourceService,
//     request::{Request, HttpRequest}, 
//     extensions::Extensions,
// };
// use ahash::AHashMap;
// use loony_service::Service;
// use std::{cell::RefCell,rc::Rc};
// use futures::executor::block_on;

#[derive(Clone)]
pub struct HttpResponse {
    pub value: String,
}
