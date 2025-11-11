use std::rc::Rc;
use ahash::AHashMap;
use std::cell::RefCell;
use futures::future::ready;
use futures::{future::Ready};
use crate::route::RouteServices;
use crate::extensions::Extensions;
use crate::resource::ResourceService;
use crate::service::{AppServiceFactory};
use loony_service::{ServiceFactory, Service};

pub struct AppFactory {
    pub services: Rc<RefCell<Vec<Box<dyn AppServiceFactory>>>>,
    // pub app_data: AppState,
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
        let mut config = RouteServices::new();
        std::mem::take(&mut *self.services.borrow_mut())
        .into_iter()
        .for_each(|mut srv| srv.register(&mut config));

        let services = config.into_services();
        let mut routes = AHashMap::new();
        services.iter().for_each(|f| {
            let g = Rc::clone(f);
            let h = g.as_ref().borrow();
            let i = h.route_name.clone();
            routes.insert(i, Rc::clone(&g));
        });
        let new_ext = self
            .extensions
            .borrow_mut()
            .take()
            .unwrap_or_else(Extensions::new);
        ready(Ok(AppHttpService {
            routes,
            extensions: new_ext
        }))
    }
}
 
pub struct AppHttpService {
    pub(crate) routes: AHashMap<String, Rc<RefCell<ResourceService>>>,
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
