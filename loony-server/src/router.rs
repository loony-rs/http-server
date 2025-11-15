use std::cell::RefCell;
use std::rc::Rc;
use loony_router::radix::RadixRouter;

use crate::{resource::FinalRouteService, route::Route, service::{AppServiceFactory, HttpServiceFactory, ServiceFactoryWrapper}};

pub struct AllRouteServices {
    route: RadixRouter,
    services: Vec<Rc<RefCell<FinalRouteService>>>
}

impl AllRouteServices {

    pub fn new() -> Self {
        Self {
            route: RadixRouter::new(),
            services: Vec::new()
        }
    }

    pub fn add_route(&mut self, path: &str, service: Rc<RefCell<FinalRouteService>>) {
        self.services.push(service);
        let index = self.services.len() - 1;
        self.route.add_route(path, index);
    }

    pub fn find_route(&self, path: &str) -> Option<Rc<RefCell<FinalRouteService>>> {
        let res = self.route.find_route(path);
        if let Some(res) = res {
            let service = Rc::clone(&self.services[res.0]);
            return Some(service);
        }
        None
    }
}

pub struct Router {
    pub services: Vec<Box<dyn AppServiceFactory>>,
}

impl Router {
    pub fn new() -> Self {
        Router { 
            services: Vec::new()
        }
    }

    pub fn route(mut self, route: Route) -> Self {
        self.services.push(Box::new(route));
        self
    }


    pub fn service<T>(mut self, factory: T) -> Self
    where 
      T: HttpServiceFactory + 'static
    {
      self.services.push(Box::new(ServiceFactoryWrapper::new(factory)));
      self
    }
}
