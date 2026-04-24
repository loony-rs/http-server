use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::{
    route::RouteServices,
    route::{BoxedRouteService, Route, RouteFutureService},
    service::{AppServiceFactory, ServiceRequest, ServiceResponse},
};
use futures::executor::block_on;
use futures::{Future, FutureExt};
use loony_service::{Service, ServiceFactory};

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
        let mut route_name = format!("{}", &self.scope);
        route_name.push_str(&self.route.path);
        let fut = self.route.new_service(());
        FinalFutureRouteService { route_name, fut }
    }
}

impl AppServiceFactory for Resource {
    fn register(&mut self, config: &mut RouteServices) {
        let a = self.new_service(());
        match block_on(a) {
            Ok(b) => config.service(b),
            Err(()) => {
                eprintln!(
                    "warning: failed to build service for resource '{}'",
                    self.scope
                );
            },
        }
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
    type Future = Pin<Box<dyn Future<Output = Result<ServiceResponse, ()>>>>;

    fn call(&mut self, req: Self::Request) -> Self::Future {
        self.service.call(req).boxed_local()
    }
}

impl Future for FinalFutureRouteService {
    type Output = Result<FinalRouteService, ()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.fut.fut.as_mut().poll(cx) {
            Poll::Ready(Ok(service)) => Poll::Ready(Ok(FinalRouteService {
                service,
                route_name: self.route_name.clone(),
            })),
            Poll::Ready(Err(())) => Poll::Ready(Err(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::extensions::Extensions;
    use crate::request::HttpRequest;
    use crate::resource::Resource;
    use crate::route::Route;
    use crate::route::RouteServices;
    use crate::service::AppServiceFactory;
    use crate::service::ServiceRequest;
    use futures::executor::block_on;
    use loony_service::Service;
    use std::rc::Rc;

    async fn index(_: String) -> String {
        "Hello World!".to_string()
    }

    #[test]
    fn resource() {
        let route = Route::new("/home").to(index);
        let mut resource = Resource::new("".to_string()).route(route);
        let mut route_services = RouteServices::new();
        resource.register(&mut route_services);

        let one = route_services.services.get(0).unwrap();
        let mut req = HttpRequest::new();
        req.uri = Some("/home".to_string());
        let ext = Extensions::new();
        let service_request = ServiceRequest {
            req,
            extensions: Rc::new(ext),
            path_params: Rc::new(vec![]),
        };

        let res = one.borrow_mut().call(service_request);
        let res = block_on(res).unwrap();
        assert!(res.0.ends_with("Hello World!"));
    }
}
