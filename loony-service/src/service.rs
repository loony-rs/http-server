//! See [`Service`](trait.Service.html) docs for information on this crate's foundational trait.

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

/// An asynchronous operation from `Request` to a `Response`.
///
/// The `Service` trait models a request/response interaction, receiving requests and returning
/// replies. You can think about a service as a function with one argument that returns some result
/// asynchronously. Conceptually, the operation looks like this:
///
/// ```rust,ignore
/// async fn(Request) -> Result<Response, Err>
/// ```
///
/// The `Service` trait just generalizes this form where each parameter is described as an
/// associated type on the trait. Services can also have mutable state that influence computation.
///
/// `Service` provides a symmetric and uniform API; the same abstractions can be used to represent
/// both clients and servers. Services describe only _transformation_ operations which encourage
/// simple API surfaces. This leads to simpler design of each service, improves test-ability and
/// makes composition easier.
///
/// ```rust,ignore
/// struct MyService;
///
/// impl Service for MyService {
///      type Request = u8;
///      type Response = u64;
///      type Error = MyError;
///      type Future = Pin<Box<Future<Output=Result<Self::Response, Self::Error>>>>;
///
///      fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { ... }
///
///      fn call(&mut self, req: Self::Request) -> Self::Future { ... }
/// }
/// ```
///
/// Sometimes it is not necessary to implement the Service trait. For example, the above service
/// could be rewritten as a simple function and passed to [fn_service](fn.fn_service.html).
///
/// ```rust,ignore
/// async fn my_service(req: u8) -> Result<u64, MyError>;
/// ```
pub trait Service {
    /// Requests handled by the service.
    type Request;

    /// Responses given by the service.
    type Response;

    /// Errors produced by the service.
    type Error;

    /// The future response value.
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    /// Process the request and return the response asynchronously.
    ///
    /// This function is expected to be callable off task. As such,
    /// implementations should take care to not call `poll_ready`. If the
    /// service is at capacity and the request is unable to be handled, the
    /// returned `Future` should resolve to an error.
    ///
    /// Calling `call` without calling `poll_ready` is permitted. The
    /// implementation must be resilient to this fact.
    fn call(&mut self, req: Self::Request) -> Self::Future;

}

/// Factory for creating `Service`s.
///
/// Acts as a service factory. This is useful for cases where new `Service`s
/// must be produced. One case is a TCP server listener. The listener
/// accepts new TCP streams, obtains a new `Service` using the
/// `ServiceFactory` trait, and uses the new `Service` to process inbound
/// requests on that new TCP stream.
///
/// `Config` is a service factory configuration type.
pub trait ServiceFactory {
    /// Requests handled by the created services.
    type Request;

    /// Responses given by the created services.
    type Response;

    /// Errors produced by the created services.
    type Error;

    /// Service factory configuration.
    type Config;

    /// The kind of `Service` created by this factory.
    type Service: Service<
        Request = Self::Request,
        Response = Self::Response,
        Error = Self::Error,
    >;

    /// Errors potentially raised while building a service.
    type InitError;

    /// The future of the `Service` instance.
    type Future: Future<Output = Result<Self::Service, Self::InitError>>;

    /// Create and return a new service asynchronously.
    fn new_service(&self, cfg: Self::Config) -> Self::Future;

}

impl<'a, S> Service for &'a mut S
where
    S: Service + 'a,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, request: Self::Request) -> S::Future {
        (**self).call(request)
    }
}

impl<S> Service for Box<S>
where
    S: Service + ?Sized,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, request: Self::Request) -> S::Future {
        (**self).call(request)
    }
}

impl<S> Service for RefCell<S>
where
    S: Service,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, request: Self::Request) -> S::Future {
        self.borrow_mut().call(request)
    }
}

impl<S> Service for Rc<RefCell<S>>
where
    S: Service,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, request: Self::Request) -> S::Future {
        (&mut (**self).borrow_mut()).call(request)
    }
}

impl<S> ServiceFactory for Rc<S>
where
    S: ServiceFactory,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Config = S::Config;
    type Service = S::Service;
    type InitError = S::InitError;
    type Future = S::Future;

    fn new_service(&self, cfg: S::Config) -> S::Future {
        self.as_ref().new_service(cfg)
    }
}

impl<S> ServiceFactory for Arc<S>
where
    S: ServiceFactory,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Config = S::Config;
    type Service = S::Service;
    type InitError = S::InitError;
    type Future = S::Future;

    fn new_service(&self, cfg: S::Config) -> S::Future {
        self.as_ref().new_service(cfg)
    }
}

/// Trait for types that can be converted to a `Service`
pub trait IntoService<T>
where
    T: Service,
{
    /// Convert to a `Service`
    fn into_service(self) -> T;
}

/// Trait for types that can be converted to a `ServiceFactory`
pub trait IntoServiceFactory<T>
where
    T: ServiceFactory,
{
    /// Convert `Self` to a `ServiceFactory`
    fn into_factory(self) -> T;
}

impl<T> IntoService<T> for T
where
    T: Service,
{
    fn into_service(self) -> T {
        self
    }
}

impl<T> IntoServiceFactory<T> for T
where
    T: ServiceFactory,
{
    fn into_factory(self) -> T {
        self
    }
}

/// Convert object of type `T` to a service `S`
pub fn into_service<T, S>(tp: T) -> S
where
    S: Service,
    T: IntoService<S>,
{
    tp.into_service()
}
