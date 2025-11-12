use crate::{route::Route, service::{AppServiceFactory, HttpServiceFactory, ServiceFactoryWrapper}};


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
