use std::rc::Rc;
use std::cell::RefCell;
use crate::{
  resource::FinalRouteService, 
  service::{AppServiceFactory, HttpServiceFactory, ServiceFactoryWrapper}
};

pub struct ServiceConfig {
  pub services:Vec<Box<dyn AppServiceFactory>>,
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
