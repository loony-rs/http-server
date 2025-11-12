use std::cell::RefCell;
use std::rc::Rc;

use loony_service::IntoServiceFactory;

use crate::app_service::AppFactory;
use crate::config::ServiceConfig;
use crate::extensions::Extensions;
use crate::route::{Route};
use crate::router::Router;
use crate::service::{AppServiceFactory};

pub struct App {
    pub extensions: Extensions,
    pub services: Vec<Box<dyn AppServiceFactory>>,
}

impl App {
    pub fn new() -> Self {
      App { 
        extensions: Extensions::new(),
        services: Vec::new()
      } 
    }

    pub fn app_data<U: 'static>(mut self, ext: U) -> Self {
        self.extensions.insert(ext);
        self
    }

    pub fn data<U: 'static>(mut self, ext: U) -> Self {
        self.extensions.insert(ext);
        self
    }

    pub fn route(mut self, route: Route) -> Self 
    {
        self.services.push(Box::new(route));
        self
    }

    pub fn configure<'a, T>(mut self, cnfg: T) -> Self where T: Fn(&mut ServiceConfig) {
        let mut configs = ServiceConfig::new();
        cnfg(&mut configs);
        self.services.extend(configs.services);
        self
    }

    pub fn routes<'a, T>(mut self, cnfg: T) -> Self where T: Fn() -> Router {
        let router = cnfg();
        self.services.extend(router.services);
        self
    }
}

impl IntoServiceFactory<AppFactory> for App {
    fn into_factory(self) -> AppFactory {
        AppFactory {
            services: Rc::new(RefCell::new(self.services)),
            extensions: RefCell::new(Some(self.extensions)),
        }
    }
}