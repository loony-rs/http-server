use std::rc::Rc;

use crate::{route::RouteServices, extensions::Extensions, request::HttpRequest};

pub trait HttpServiceFactory {
    fn register(self,  config: &mut RouteServices);
}

pub trait AppServiceFactory {
    fn register(&mut self, config: &mut RouteServices);
}

pub trait RouteServiceFactory {
    fn register(&mut self, config: &mut RouteServices);
}


#[derive(Clone)]
pub struct ServiceRequest{
    pub req: HttpRequest,
    pub extensions: Rc<Extensions>
}

// #[derive(Clone)]
// pub struct ServiceResponse(pub HttpResponse);

#[derive(Clone)]
pub struct ServiceResponse(pub String);

pub(crate) struct ServiceFactoryWrapper<T> {
    factory: Option<T>,
}

impl<T> ServiceFactoryWrapper<T> {
    pub fn new(factory: T) -> Self {
        Self {
            factory: Some(factory),
        }
    }
}

impl<T> AppServiceFactory for ServiceFactoryWrapper<T>
where
    T: HttpServiceFactory,
{
    fn register(&mut self, config: &mut RouteServices) {
        if let Some(item) = self.factory.take() {
            item.register(config)
        }
    }
}