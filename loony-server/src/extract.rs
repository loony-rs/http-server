use std::future::Future;
use std::future::{Ready, ready};
use std::marker::PhantomData;
use crate::service::{ServiceRequest, ServiceResponse};
use pin_project::pin_project;
use futures_util::ready as fut_ready;
use std::{pin::Pin, task::Poll};

use loony_service::{Service, ServiceFactory};

pub trait FromPathSegments: Clone {
    fn from_segments(segments: &[&str]) -> Option<Self>
    where
        Self: Sized;
}


impl FromPathSegments for i32 {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 2 { return None; }
        let id = segments[0].parse().ok()?;
        Some(id)
    }
}

impl FromPathSegments for (i32, String) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 2 { return None; }
        let id = segments[0].parse().ok()?;
        let name = segments[1].to_string();
        Some((id, name))
    }
}

impl FromPathSegments for (i32, i32) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 2 { return None; }
        Some((segments[0].parse().ok()?, segments[1].parse().ok()?))
    }
}

impl FromPathSegments for (String, String) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 2 { return None; }
        Some((segments[0].to_string(), segments[1].to_string()))
    }
}

impl FromPathSegments for (i32, String, String) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 3 { return None; }
        Some((segments[0].parse().ok()?, segments[1].to_string(), segments[2].to_string()))
    }
}

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
pub struct Path<T>(pub T);

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
        return ready(Ok((Data(a.clone()), "".to_string(),)));
    }
}

impl<T, P> FromRequest for (Data<T>, Path<P>,)
where
    T: Clone + Send + Sync + 'static,
    P: FromPathSegments + Clone,
{
    type Future = Ready<Result<(Data<T>, Path<P>), ()>>;

    fn from_request(req: &ServiceRequest) -> Self::Future {
        let a = req.extensions.get::<T>().unwrap();
        let path = req.req.uri.clone().unwrap();
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match P::from_segments(&segments[2..]) {
            Some(p) => ready(Ok((Data(a.clone()), Path(p)))),
            None => ready(Err(())),
        }
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

// macro_rules! impl_from_request_for_path {
//     ($(($($t:ty),+))+) => {
//         $(
//             impl<T, $($t),+> FromRequest for (Data<T>, Path<($($t),+)>)
//             where
//                 T: Clone + Send + Sync + 'static,
//                 $($t: FromStr + Clone + Send + Sync + 'static,)+
//             {
//                 type Error = ();
//                 type Future = Ready<Result<Self, Self::Error>>;

//                 fn from_request(req: &ServiceRequest) -> Self::Future {
//                     let data = match req.extensions().get::<T>() {
//                         Some(d) => Data(d.clone()),
//                         None => return ready(Err(())),
//                     };

//                     let path = req.path();
//                     let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
                    
//                     const EXPECTED_COUNT: usize = [0 $(+ replace_expr!($t 1))+];
//                     if segments.len() < EXPECTED_COUNT {
//                         return ready(Err(()));
//                     }

//                     let mut iter = segments.iter();
//                     $(
//                         let $t: $t = match iter.next().unwrap().parse() {
//                             Ok(val) => val,
//                             Err(_) => return ready(Err(())),
//                         };
//                     )+

//                     ready(Ok((data, Path(($($t),+)))))
//                 }
//             }
//         )+
//     };
// }

// macro_rules! replace_expr {
//     ($_t:ty, $sub:expr) => { $sub };
// }
