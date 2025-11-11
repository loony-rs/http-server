use loony_service::{ServiceFactory};
use crate::{
    route::Route, 
    route::RouteServices, 
    resource::{Resource, ResourceService, FutureResourceService}, 
    service::{AppServiceFactory, HttpServiceFactory, ServiceRequest, ServiceResponse}
};

pub type BoxedResourceServiceFactory = Box<
    dyn ServiceFactory<
        Request = ServiceRequest, 
        Response = ServiceResponse, 
        Error = (), 
        Service = ResourceService,
        Config=(),
        InitError=(),
        Future = FutureResourceService
    >
>;

pub struct Scope {
    pub scope: String,
    pub services: Vec<Box<dyn AppServiceFactory>>,
}

impl Scope {
    pub fn new(scope: &str) -> Self {
        Scope {
            scope: scope.to_owned(),
            services: Vec::new()
        }
    }

    pub fn route(mut self, route: Route) -> Self {
        self.services.push(Box::new(Resource::new(self.scope.clone()).route(route)));
        self
    }
}

impl HttpServiceFactory for Scope {
    fn register(self, config: &mut RouteServices) {
        self.services.into_iter().for_each(|mut f| f.register(config));
    }
}

// pub struct ScopeFactory {
//     services: Vec<ResourceService>
// }
