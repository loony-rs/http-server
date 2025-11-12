use std::{
    cell::RefCell, future::Future, pin::Pin, rc::Rc, task::{Context, Poll}
};
use async_std::task::block_on;
use loony_service::{
    Service,
    ServiceFactory
};
use crate::{
    extract::{Extract, FromRequest}, 
    handler::{Factory, Handler}, 
    resource::{Resource, FinalRouteService}, responder::Responder, scope::Scope, service::{AppServiceFactory, HttpServiceFactory, ServiceFactoryWrapper, ServiceRequest, ServiceResponse}
};

#[derive(Clone)]
pub enum Method {
  GET,
  POST,
}

pub type BoxedRouteService = Box<
    dyn Service<
        Request=ServiceRequest,
        Response=ServiceResponse,
        Error=(),
        Future=Pin<Box<dyn Future<Output=Result<ServiceResponse, ()>>>>
    >
>;

pub type BoxedRouteServiceFactory = Box<
    dyn ServiceFactory<
        Request=ServiceRequest,
        Response=ServiceResponse,
        Error=(),
        Service=BoxedRouteService,
        Future=Pin<Box<dyn Future<Output=Result<BoxedRouteService, ()>>>>,
        Config=(),
        InitError=()
    >
>;


pub type BoxService = Pin<
    Box<
        dyn Future<Output=Result<BoxedRouteService, ()>>
    >
>;

// #[derive(Clone)]
pub struct Route {
    pub path: String,
    pub service: BoxedRouteServiceFactory,
    pub method: Method,
}

impl<'route> Route {
    pub fn new(path: &str) -> Route {
        Route {
            path: path.to_owned(),
            service: Box::new(
                RouteServiceWrapper::new(
                    Extract::new(
                        Handler::new(default)
                    )
                )
            ),
            method: Method::GET,
        }
    }

    pub fn to<T, P, R, O>(mut self, factory: T) -> Self 
    where 
        T: Factory<P, R, O> + Clone + 'static, 
        P: FromRequest + 'static,
        R: Future<Output=O> + 'static, 
        O: Responder + 'static, 
    {
        
        let service = Box::new(RouteServiceWrapper::new(Extract::new(Handler::new(factory))));
        self.service = service;
        self
    }

    pub fn method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }
}

impl AppServiceFactory for Route {
    fn register(&mut self, config: &mut RouteServices) {
        let x = self.service.new_service(());
        let res = block_on(x).unwrap();
        config.service(FinalRouteService { service: res, route_name: self.path.clone() });
    }
}

pub struct RouteService {
    service: BoxedRouteService,
}

pub struct RouteServices {
  pub services: Vec<Rc<RefCell<FinalRouteService>>>
}

impl RouteServices {
  pub fn new() -> Self {
    RouteServices {
      services: Vec::new(),
    }
  }

  pub fn service(&mut self, service: FinalRouteService) {
    self.services.push(Rc::new(RefCell::new(service)));
  }

  pub fn into_services(self) -> Vec<Rc<RefCell<FinalRouteService>>> {
    self.services
  }
}

impl Service for RouteService {
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output=Result<Self::Response, ()>>>>;

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        self.service.call(req)
    }
}

impl ServiceFactory for Route {
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Service = RouteService;
    type InitError = ();
    type Config = ();
    type Future = RouteFutureService;

    fn new_service(&self, _: ()) -> Self::Future {
        let fut = self.service.new_service(());
        RouteFutureService { fut }
    }
}

#[pin_project::pin_project]
pub struct RouteFutureService {
    #[pin]
    pub fut: BoxService,
}

impl Future for RouteFutureService {
    type Output = Result<RouteService, ()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.fut.poll(cx)? {
            Poll::Ready(service) => Poll::Ready(Ok(RouteService {
                service
            })),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct RouteServiceWrapper<T> 
where
    T: ServiceFactory<Request = ServiceRequest>
{
    factory: T,
}

impl<T> RouteServiceWrapper<T> 
where
    T: ServiceFactory<Request = ServiceRequest>
{
    pub fn new(factory: T) -> Self {
        RouteServiceWrapper {
            factory,
        }
    }
}

impl<T> ServiceFactory for RouteServiceWrapper<T> 
where
    T: ServiceFactory<
        Config = (),
        Request = ServiceRequest,
        Response = ServiceResponse,
        Error = (),
        InitError = ()
    >,
    T::Future: 'static,
    T::Service: 'static,
    <T::Service as Service>::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Config = ();
    type Error = ();
    type InitError = ();
    type Service = BoxedRouteService;
    type Future = Pin<Box<dyn Future<Output=Result<Self::Service, ()>>>>;

    fn new_service(&self, _: Self::Config) -> Self::Future {
        let fut = self.factory.new_service(());
        Box::pin(MakeFut { fut })
    }
}

#[pin_project::pin_project]
struct MakeFut<F> {
    #[pin]
    fut: F
}

impl<F, S> Future for MakeFut<F>
where
    F: Future<Output = Result<S, ()>>,
    S: Service<
        Request = ServiceRequest,
        Response = ServiceResponse,
        Error = (),
    > + 'static,
    <S as Service>::Future: 'static,
{
    type Output = Result<BoxedRouteService, ()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let fut = self.project();
        match fut.fut.poll(cx) {
            Poll::Ready(Ok(svc)) => {
                let service: BoxedRouteService = Box::new(RouteHandlerService { factory: svc });
                Poll::Ready(Ok(service))
            }
            Poll::Ready(Err(_)) => Poll::Ready(Err(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pin_project::pin_project]
struct MakeService<F> {
    #[pin]
    inner: F,
}

struct RouteHandlerService<T: Service> {
    factory:T 
}

impl<T> Service for RouteHandlerService<T> 
where
    T::Future: 'static,
    T: Service<
        Request = ServiceRequest,
        Response = ServiceResponse,
        Error = (),
    >,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output=Result<ServiceResponse, ()>>>>;

    fn call(&mut self, req: Self::Request) -> Self::Future {
        let fut = self.factory.call(req);
        Box::pin(CallFut { inner: fut })
    }
}

#[pin_project::pin_project]
struct CallFut<F> {
    #[pin]
    inner: F,
}

impl<F> Future for CallFut<F>
where
    F: Future<Output = Result<ServiceResponse, ()>>,
{
    type Output = Result<ServiceResponse, ()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().inner.poll(cx)? {
            Poll::Ready(service) => Poll::Ready(Ok(service)),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn method(path: &str, method: Method) -> Route {
    Route::new(path).method(method)
}

pub fn get(path: &str) -> Route {
    method(path, Method::GET)
}

pub fn post(path: &str) -> Route {
    method(path, Method::POST)
}

pub fn scope(scope: &str) -> Scope {
  Scope::new(scope)
}

pub async fn default() -> String {
    "".to_string()
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use crate::extensions::Extensions;
    use crate::request::HttpRequest;
    use std::rc::Rc;
    use super::*;

    async fn index(_: String) -> String {
        "Hello World!".to_string()
    }

    #[test]
    fn route() {
        let route = Route::new("/home").to(index);
        let a = route.new_service(());
        let mut b = block_on(a).unwrap();

        let ext = Extensions::new();
        let req = HttpRequest::new();
        let sr = ServiceRequest {
            req,
            extensions: Rc::new(ext),
        };

        let c = b.call(sr);
        let d = block_on(c).unwrap();
        let e = d.0;
        assert_eq!("Hello World!".to_string(), e);
    }
}