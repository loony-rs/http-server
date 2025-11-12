use std::{
  pin::Pin,
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
}

impl Resource {
  pub fn new(scope: String) -> Self {
    Resource {
      scope,
      route: Route::new(""),
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
    type Service = FinalRouteService;
    type Future = FinalFutureRouteService;
    type InitError = ();
    type Config = ();
 
    fn new_service(&self, _: ()) -> Self::Future {
        let mut route_name = self.scope.clone();
        route_name.push_str(&self.route.path);
        let fut = self.route.new_service(());
        FinalFutureRouteService {
          route_name,
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
pub struct FinalFutureRouteService {
    #[pin]
    pub fut: RouteFutureService,
    pub route_name: String,
}
pub struct FinalRouteService {
    pub service: BoxedRouteService,
    pub route_name: String,
}

impl Service for FinalRouteService {
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output=Result<ServiceResponse, ()>>>>;

    fn call(&mut self, req: Self::Request) -> Self::Future {
        self.service.call(req).boxed_local()
    }
}

impl Future for FinalFutureRouteService {
    type Output = Result<FinalRouteService, ()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.fut.fut.as_mut().poll(cx) {
          Poll::Ready(service) => Poll::Ready(Ok(FinalRouteService {
              service: service.unwrap(),
              route_name: self.route_name.clone(),
          })),
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
      // let mut a= rs.route_service.borrow_mut();
      // if let Some(mut c) = a.take() {
      //   c.call(sr);
      // }
    }
}