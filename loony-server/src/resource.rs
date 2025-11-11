use std::{
  pin::Pin, 
  rc::Rc,
  cell::RefCell,
  task::{Context, Poll},
};

use crate::{
  route::RouteServices, 
  route::{
    BoxedRouteService, 
    Route, 
    RouteFutureService
  }, service::{
    ServiceRequest, 
    ServiceResponse,
    AppServiceFactory,
  }};
use async_std::task::block_on;
use futures::{Future, FutureExt};
use loony_service::{ServiceFactory, Service};

pub struct Resource {
  scope: String,
  route: Route,
  route_service: Rc<RefCell<Option<ResourceService>>>
}

impl Resource {
  pub fn new(scope: String) -> Self {
    Resource {
      scope,
      route: Route::new(""),
      route_service: Rc::new(RefCell::new(None)),
    }
  }

  pub fn route(mut self, route: Route) -> Self {
    self.route = route;
    self
  }
}

impl ServiceFactory for Resource {
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Service = ResourceService;
    type Future = FutureResourceService;
    type InitError = ();
    type Config = ();
 
    fn new_service(&self, _: ()) -> Self::Future {
        let mut route_name = self.scope.clone();
        let a: Vec<&str> = self.route.path.split("::").collect();
        let mut b = a.iter();
        if let Some(data) = b.next() {
          route_name.push_str(data);
        }
        let mut route_query_param = String::new();
        if let Some(data) = b.next() {
          route_query_param.push_str(data);
        }
        let fut = self.route.new_service(());
        FutureResourceService {
          len: route_name.len() as u16,
          route_name,
          route_query_param,
          fut,
        }
    }
}

impl AppServiceFactory for Resource {
  fn register(&mut self, config: &mut RouteServices) {
    let a = self.new_service(());
    let b = block_on(a).unwrap();
    config.service(b);
  }
}

#[pin_project::pin_project]
pub struct FutureResourceService {
    #[pin]
    pub fut: RouteFutureService,
    pub route_name: String,
    pub route_query_param: String,
    pub len: u16,
}
pub struct ResourceService {
    pub service: BoxedRouteService,
    pub route_name: String,
    pub route_query_param: String,
    pub len: u16,
}

impl Service for ResourceService {
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output=Result<ServiceResponse, ()>>>>;

    fn call(&mut self, req: Self::Request) -> Self::Future {
        self.service.call(req).boxed_local()
    }
}

impl Future for FutureResourceService {
    type Output = Result<ResourceService, ()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.fut.fut.as_mut().poll(cx) {
          Poll::Ready(service) => {
            let a = Poll::Ready(Ok(ResourceService {
                service: service.unwrap(),
                route_name: self.route_name.clone(),
                route_query_param: self.route_query_param.clone(),
                len: self.len,
            }));
            return a;
          },
          Poll::Pending => Poll::Pending
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::extensions::Extensions;
    use crate::request::HttpRequest;
    use crate::route::Route;
    use crate::service::AppServiceFactory;
    use crate::service::ServiceRequest;
    use crate::resource::Resource;
    use loony_service::Service;
    use std::rc::Rc;
    use crate::route::RouteServices;

    async fn index(_: String) -> String {
        "Hello World!".to_string()
    }

    #[test]
    fn resource() {
      let r = Route::new("/home");
      let r = r.to(index);
      let rs = Resource::new("".to_string());
      let mut rs = rs.route(r);
      let mut a_ser = RouteServices::new();
      rs.register(&mut a_ser);
      let req = HttpRequest::new();
      let ext = Extensions::new();
      let sr = ServiceRequest { req, extensions:Rc::new(ext) };
      let mut a= rs.route_service.borrow_mut();
      if let Some(mut c) = a.take() {
        c.call(sr);
      }
    }
}