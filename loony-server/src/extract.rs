use crate::service::{ServiceRequest, ServiceResponse};
use futures_util::ready as fut_ready;
use pin_project::pin_project;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::future::{Ready, ready};
use std::marker::PhantomData;
use std::{pin::Pin, task::Poll};

use loony_service::{Service, ServiceFactory};

// ---------------------------------------------------------------------------
// Path parameter extraction
// ---------------------------------------------------------------------------

pub trait FromPathSegments: Clone {
    fn from_segments(segments: &[&str]) -> Option<Self>
    where
        Self: Sized;
}

impl FromPathSegments for i32 {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        segments.first()?.parse().ok()
    }
}

impl FromPathSegments for String {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        segments.first().map(|s| s.to_string())
    }
}

impl FromPathSegments for (i32, String) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 2 {
            return None;
        }
        Some((segments[0].parse().ok()?, segments[1].to_string()))
    }
}

impl FromPathSegments for (i32, i32) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 2 {
            return None;
        }
        Some((segments[0].parse().ok()?, segments[1].parse().ok()?))
    }
}

impl FromPathSegments for (String, String) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 2 {
            return None;
        }
        Some((segments[0].to_string(), segments[1].to_string()))
    }
}

impl FromPathSegments for (i32, String, String) {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        if segments.len() < 3 {
            return None;
        }
        Some((
            segments[0].parse().ok()?,
            segments[1].to_string(),
            segments[2].to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// FromRequest trait
// ---------------------------------------------------------------------------

pub trait FromRequest: Clone {
    type Future: Future<Output = Result<Self, ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future;
}

// () — handlers that take no arguments
impl FromRequest for () {
    type Future = Ready<Result<(), ()>>;
    fn from_request(_: &ServiceRequest) -> Self::Future {
        ready(Ok(()))
    }
}

// String — yields the raw request URI
impl FromRequest for String {
    type Future = Ready<Result<String, ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        match req.req.uri.clone() {
            Some(uri) => ready(Ok(uri)),
            None => ready(Err(())),
        }
    }
}

// ---------------------------------------------------------------------------
// Data<T> — application-wide shared state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Data<T>(pub T);

impl<T> FromRequest for Data<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Future = Ready<Result<Data<T>, ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        match req.extensions.get::<T>() {
            Some(data) => ready(Ok(Data(data.clone()))),
            None => ready(Err(())),
        }
    }
}

// ---------------------------------------------------------------------------
// Path<T> — typed path parameters
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Path<T>(pub T);

// ---------------------------------------------------------------------------
// Json<T> — deserialize the request body as JSON
// ---------------------------------------------------------------------------

/// Extractor that deserialises the request body as JSON.
///
/// The handler receives an `Err(())` (→ 500) if the body is missing or
/// cannot be deserialised.  Step 10 will replace the opaque `()` error with
/// a proper `400 Bad Request` response.
#[derive(Clone)]
pub struct Json<T>(pub T);

impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned + Clone + 'static,
{
    type Future = Ready<Result<Json<T>, ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        let body = match req.req.body.as_ref() {
            Some(b) => b,
            None => return ready(Err(())),
        };
        match serde_json::from_slice::<T>(body) {
            Ok(value) => ready(Ok(Json(value))),
            Err(_) => ready(Err(())),
        }
    }
}

// ---------------------------------------------------------------------------
// Tuple extractors
// ---------------------------------------------------------------------------

impl FromRequest for (String,) {
    type Future = Ready<Result<(String,), ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        match req.req.uri.clone() {
            Some(uri) => ready(Ok((uri,))),
            None => ready(Err(())),
        }
    }
}

impl<T> FromRequest for (Data<T>,)
where
    T: Clone + Send + Sync + 'static,
{
    type Future = Ready<Result<(Data<T>,), ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        match req.extensions.get::<T>() {
            Some(data) => ready(Ok((Data(data.clone()),))),
            None => ready(Err(())),
        }
    }
}

impl<T> FromRequest for (Data<T>, String)
where
    T: Clone + Send + Sync + 'static,
{
    type Future = Ready<Result<(Data<T>, String), ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        let data = match req.extensions.get::<T>() {
            Some(d) => Data(d.clone()),
            None => return ready(Err(())),
        };
        let uri = match req.req.uri.clone() {
            Some(u) => u,
            None => return ready(Err(())),
        };
        ready(Ok((data, uri)))
    }
}

impl<T, P> FromRequest for (Data<T>, Path<P>)
where
    T: Clone + Send + Sync + 'static,
    P: FromPathSegments + Clone,
{
    type Future = Ready<Result<(Data<T>, Path<P>), ()>>;

    fn from_request(req: &ServiceRequest) -> Self::Future {
        let data = match req.extensions.get::<T>() {
            Some(d) => Data(d.clone()),
            None => return ready(Err(())),
        };
        let path = match req.req.uri.as_ref() {
            Some(p) => p.clone(),
            None => return ready(Err(())),
        };

        // Collect non-empty segments and skip the scope prefix segments.
        // The scope prefix length is determined by the route name stored on
        // the service; for now we drop the first two segments (scope + empty
        // leading slash artefact).  Step 4 will make this robust.
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() < 2 {
            return ready(Err(()));
        }
        match P::from_segments(&segments[2..]) {
            Some(p) => ready(Ok((data, Path(p)))),
            None => ready(Err(())),
        }
    }
}

impl<T, U> FromRequest for (Data<T>, Json<U>)
where
    T: Clone + Send + Sync + 'static,
    U: DeserializeOwned + Clone + 'static,
{
    type Future = Ready<Result<(Data<T>, Json<U>), ()>>;
    fn from_request(req: &ServiceRequest) -> Self::Future {
        let data = match req.extensions.get::<T>() {
            Some(d) => Data(d.clone()),
            None => return ready(Err(())),
        };
        let body = match req.req.body.as_ref() {
            Some(b) => b,
            None => return ready(Err(())),
        };
        match serde_json::from_slice::<U>(body) {
            Ok(value) => ready(Ok((data, Json(value)))),
            Err(_) => ready(Err(())),
        }
    }
}

// ---------------------------------------------------------------------------
// Extract service infrastructure (unchanged from original)
// ---------------------------------------------------------------------------

pub struct Extract<T: FromRequest, S> {
    service: S,
    _t: PhantomData<T>,
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
    S: Service<Request = (T, ServiceRequest), Response = ServiceResponse, Error = ()> + Clone,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Config = ();
    type Service = ExtractService<T, S>;
    type InitError = ();
    type Future = Ready<Result<Self::Service, ()>>;

    fn new_service(&self, _: Self::Config) -> Self::Future {
        ready(Ok(ExtractService {
            service: self.service.clone(),
            _t: PhantomData,
        }))
    }
}

pub struct ExtractService<T, S> {
    service: S,
    _t: PhantomData<T>,
}

impl<T: FromRequest, S> Service for ExtractService<T, S>
where
    S: Service<Request = (T, ServiceRequest), Response = ServiceResponse, Error = ()> + Clone,
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
pub struct ExtractResponse<T: FromRequest, S: Service> {
    req: ServiceRequest,
    service: S,
    #[pin]
    fut: T::Future,
    #[pin]
    fut_s: Option<S::Future>,
}

impl<T: FromRequest, S: Service> Future for ExtractResponse<T, S>
where
    S: Service<Request = (T, ServiceRequest), Response = ServiceResponse, Error = ()>,
{
    type Output = Result<ServiceResponse, ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();
        if let Some(fut) = this.fut_s.as_pin_mut() {
            return fut.poll(cx);
        }

        match fut_ready!(this.fut.poll(cx)) {
            Err(_) => Poll::Ready(Err(())),
            Ok(data) => {
                let l = this.service.call((data, this.req.clone()));
                self.as_mut().project().fut_s.set(Some(l));
                self.poll(cx)
            },
        }
    }
}
