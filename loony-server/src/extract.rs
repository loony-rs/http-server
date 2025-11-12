use std::future::Future;
use std::future::{Ready, ready};
use std::marker::PhantomData;
use crate::service::{ServiceRequest, ServiceResponse};
use pin_project::pin_project;
use futures_util::ready as fut_ready;
use std::{pin::Pin, task::Poll};

use loony_service::{Service, ServiceFactory};

pub trait FromRequest: Clone {
    type Future: Future<Output=Result<Self, ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future;
}

impl FromRequest for () {
    type Future = Ready<Result<(), ()>>;
    fn from_request(_: &ServiceRequest) -> Self::Future {
        ready(Ok(()))
    }
}

impl FromRequest for (String, ) {
    type Future = Ready<Result<(String,), ()>>;
    fn from_request(_: &ServiceRequest) -> Self::Future {
        ready(Ok(("".to_string(), )))
    }
}

impl FromRequest for String {
    type Future = Ready<Result<String, ()>>;

    fn from_request(req: &ServiceRequest) -> Self::Future {
        ready(Ok(req.req.uri.clone().unwrap()))
    }
}

#[derive(Clone)]
pub struct Data<T>(pub T);

#[derive(Clone)]
pub struct Path(pub i32, pub String);

// impl<T> FromRequest for Data<T>
// where
//     T: Clone + Send + Sync + 'static,
// {
//     type Future = Ready<Result<Data<T>, ()>>;
//     fn from_request(req: &ServiceRequest) -> Self::Future {
//         let a = req.0.extensions.get::<T>().unwrap();
//         ready(Ok(Data(a.clone())))
//     }
// }

impl<T> FromRequest for Data<T> 
where
    T: Clone + Send + Sync + 'static,
{
    type Future = Ready<Result<Data<T>, ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        let a = req.extensions.get::<T>().unwrap();
        ready(Ok(Data(a.clone())))
    }
}

impl<T> FromRequest for (Data<T>, ) 
where
    T: Clone + Send + Sync + 'static,
{
    type Future = Ready<Result<(Data<T>, ), ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        let a = req.extensions.get::<T>().unwrap();
        ready(Ok((Data(a.clone()), )))
    }
}

impl<T> FromRequest for (Data<T>, String,) 
where
    T: Clone + Send + Sync + 'static,
{
    type Future = Ready<Result<(Data<T>, String,), ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        let a = req.extensions.get::<T>().unwrap();
        // let b = &req.req.params;
        return ready(Ok((Data(a.clone()), "".to_string(),)));
    }
}

impl<T> FromRequest for (Data<T>, Path,)
where
    T: Clone + Send + Sync + 'static
{
    type Future = Ready<Result<(Data<T>, Path,), ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        let a = req.extensions.get::<T>().unwrap();
        let b = 1;
        let c = req.req.uri.clone();
        // let b = &req.req.params;
        return ready(Ok((Data(a.clone()), Path(b, c.unwrap()),)));
    }
}

pub struct Extract<T: FromRequest, S> {
    service: S,
    _t: PhantomData<T>
}

impl<T: FromRequest, S> Extract<T, S> {
    pub fn new(service: S) -> Self {
        Extract {
            service,
            _t: PhantomData,
        }
    }
}

impl<T: FromRequest, S> ServiceFactory for Extract<T, S>
where 
    S: Service<
        Request=(T, ServiceRequest),
        Response=ServiceResponse,
        Error=()
    > + Clone
{
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Config = ();
    type Service = ExtractService<T, S>;
    type InitError = ();
    type Future = Ready<Result<Self::Service, ()>>;

    fn new_service(&self, _: Self::Config) -> Self::Future {
        let a= ExtractService {
            service: self.service.clone(),
            _t: PhantomData,
        };
        ready(Ok(a))
    }
}

pub struct ExtractService<T, S> {
    service: S,
    _t: PhantomData<T>
}

impl<T: FromRequest, S> Service for ExtractService<T, S> 
where 
    S: Service<
        Request=(T, ServiceRequest),
        Response=ServiceResponse,
        Error=()
    > + Clone
{
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Future = ExtractResponse<T, S>;

    fn call(&mut self, req: Self::Request) -> Self::Future {
        ExtractResponse {
            req: req.clone(),
            service: self.service.clone(),
            fut: T::from_request(&req),
            fut_s: None,
        }
    }
}

#[pin_project]
pub struct ExtractResponse <T: FromRequest, S: Service> {
    req: ServiceRequest,
    service: S,
    #[pin]
    fut: T::Future,
    #[pin]
    fut_s: Option<S::Future>,
}

impl<T: FromRequest, S: Service> Future for ExtractResponse<T, S> 
where
    S: Service<
        Request = (T, ServiceRequest),
        Response = ServiceResponse,
        Error=()
    >,
{
    type Output = Result<ServiceResponse, ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();
        if let Some(fut) = this.fut_s.as_pin_mut() {
            return match fut.poll(cx) {
                Poll::Ready(res) => {
                    Poll::Ready(res)
                }
                Poll::Pending => Poll::Pending,
            };
        }

        match fut_ready!(this.fut.poll(cx)) {
            Err(_) => {
                Poll::Ready(Err(()))
            }
            Ok(data) => {
                let l = this.service.call((data, this.req.clone()));
                self.as_mut().project().fut_s.set(Some(l));
                self.poll(cx)
            }
        }
    }
}


