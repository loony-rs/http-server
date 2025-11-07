use std::rc::Rc;
use std::cell::RefCell;
use crate::{
  resource::ResourceService, 
  service::{AppServiceFactory, HttpServiceFactory, ServiceFactoryWrapper}
};

pub struct ServiceConfig {
  pub services:Vec<Box<dyn AppServiceFactory>>,
}

pub struct RouteService {
  pub services: Vec<Rc<RefCell<ResourceService>>>
}

impl RouteService {
  pub fn new() -> Self {
    RouteService {
      services: Vec::new(),
    }
  }

  pub fn service(&mut self, service: ResourceService) {
    self.services.push(Rc::new(RefCell::new(service)));
  }

  pub fn into_services(self) -> Vec<Rc<RefCell<ResourceService>>> {
    self.services
  }
}

impl ServiceConfig {
  pub fn new() -> Self {
    ServiceConfig {
      services: Vec::new(),
    }
  }
	
	pub fn service<T>(&mut self, factory: T) 
  where 
    T: HttpServiceFactory + 'static
  {
    self.services.push(Box::new(ServiceFactoryWrapper::new(factory)));
  }
}
