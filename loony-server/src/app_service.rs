use std::rc::Rc;
use ahash::AHashMap;
use std::cell::RefCell;
use futures::future::ready;
use futures::{future::Ready};
use crate::route::RouteServices;
use crate::extensions::Extensions;
use crate::resource::FinalRouteService;
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

        let route_services = route_services.into_services();
        let mut routes = AHashMap::new();
        route_services.iter().for_each(|f| {
            routes.insert(f.borrow().route_name.clone(), Rc::clone(f));
        });
        let extensions = self
            .extensions
            .borrow_mut()
            .take()
            .unwrap_or_else(Extensions::new);
        ready(Ok(AppHttpService {
            routes,
            extensions
        }))
    }
}
 
pub struct AppHttpService {
    pub(crate) routes: AHashMap<String, Rc<RefCell<FinalRouteService>>>,
    pub(crate) extensions: Extensions,
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
