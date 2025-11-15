use std::rc::Rc;
use std::cell::RefCell;
use futures::future::ready;
use futures::{future::Ready};
use std::collections::HashMap;
use crate::route::RouteServices;
use crate::extensions::Extensions;
use crate::resource::FinalRouteService;
use crate::router::AllRouteServices;
use crate::service::{AppServiceFactory};
use loony_service::{ServiceFactory, Service};

pub struct AppFactory {
    pub services: Rc<RefCell<Vec<Box<dyn AppServiceFactory>>>>,
    pub extensions: RefCell<Option<Extensions>>,
}

impl ServiceFactory for AppFactory {
    type Request = ();

    type Response = ();

    type Error = ();

    type Config = ();

    type Service = AppHttpService;

    type InitError = ();

    type Future = Ready<Result<AppHttpService, ()>>;

    fn new_service(&self, _: Self::Config) -> Self::Future {
        let mut route_services = RouteServices::new();
        std::mem::take(&mut *self.services.borrow_mut())
        .into_iter()
        .for_each(|mut srv| srv.register(&mut route_services));
        let mut radix_router = AllRouteServices::new();
        let route_services = route_services.into_services();
        println!("{}", route_services.len());
        // let mut routes = AHashMap::new();
        route_services.iter().for_each(|f| {
            let route = f.borrow().route_name.clone();
            radix_router.add_route(&route, Rc::clone(&f));
            // let segments: Vec<&str> = route.split('/').filter(|s| !s.is_empty())
            // .filter(|s| !s.contains(":")).collect();
            // let uri = segments.join("");
            // routes.insert(uri, Rc::clone(f));
        });
        let extensions = self
            .extensions
            .borrow_mut()
            .take()
            .unwrap_or_else(Extensions::new);
        ready(Ok(AppHttpService {
            route: radix_router,
            extensions,
        }))
    }
}
 
pub struct AppHttpService {
    // pub(crate) routes: AHashMap<String, Rc<RefCell<FinalRouteService>>>,
    pub(crate) extensions: Extensions,
    pub(crate) route: AllRouteServices,
}

impl Service for AppHttpService {
    type Request = ();

    type Response = ();

    type Error = ();

    type Future = Ready<Result<(), ()>>;

    fn call(&mut self, _: Self::Request) -> Self::Future {
        ready(Ok(()))
    }
}
