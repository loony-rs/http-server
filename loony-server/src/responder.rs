use std::future::{Ready, ready, Future};
use crate::{response::HttpResponse, service::{ServiceRequest, ServiceResponse}};

pub trait Responder {
    type Future: Future<Output=ServiceResponse>;
    fn respond(&self, req: &ServiceRequest) -> Self::Future;
}

impl Responder for String {
    type Future = Ready<ServiceResponse>;

    fn respond(&self, _: &ServiceRequest) -> Self::Future {
        let r = ServiceResponse(HttpResponse{value:self.clone()});
        ready(r)
    }
}

impl Responder for Result<String, String> {
    type Future = Ready<ServiceResponse>;

    fn respond(&self, _: &ServiceRequest) -> Self::Future {
        let x = self.clone().unwrap();
        let r = ServiceResponse(HttpResponse{value:x});
        ready(r)
    }
}

impl Responder for HttpResponse {
    type Future = Ready<ServiceResponse>;

    fn respond(&self, _: &ServiceRequest) -> Self::Future {
        let r = ServiceResponse(self.clone());
        ready(r)
    }
}