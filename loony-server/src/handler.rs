use std::future::Future;
use std::marker::PhantomData;
use std::{ pin::Pin, task::Poll};

use crate::responder::Responder;
use loony_service::{Service};
use crate::service::{ServiceRequest, ServiceResponse};

pub trait Factory<P, R, O>: Clone + 'static 
where 
    R: Future<Output=O>, 
    O: Responder,
{
    fn call(&self, param: P) -> R;
}

impl<T, R, O> Factory<(), R, O> for T 
where
    T: Fn() -> R + Clone + 'static,
    R: Future<Output=O>,
    O: Responder 
{
    fn call(&self, _: ()) -> R {
        (self)()
    }
}

impl<T, PA, R, O> Factory<(PA,), R, O> for T 
where
    T: Fn(PA,) -> R + Clone + 'static,
    R: Future<Output=O>,
    O: Responder 
{
    fn call(&self, (pa,): (PA,)) -> R {
        (self)(pa)
    }
}

impl<T, PA, PB, R, O> Factory<(PA,PB), R, O> for T 
where
    T: Fn(PA,PB,) -> R + Clone + 'static,
    R: Future<Output=O>,
    O: Responder 
{
    fn call(&self, (pa,pb,): (PA,PB)) -> R {
        (self)(pa, pb)
    }
}

pub struct Handler<T, P, R, O> 
where
    T: Factory<P, R, O>,
    R: Future<Output=O>,
    O: Responder,
{
    factory: T, 
    _t: PhantomData<(P, R, O)>
}

impl<T, P, R, O> Handler<T, P, R, O> 
where
    T: Factory<P, R, O>,
    R: Future<Output=O>,
    O: Responder,
{
    pub fn new(factory: T) -> Self {
        Handler {
            factory,
            _t: PhantomData,
        }
    }
}

impl<T, P, R, O> Clone for Handler<T, P, R, O> 
where
    T: Factory<P, R, O>,
    R: Future<Output=O>,
    O: Responder,
{
    fn clone(&self) -> Self {
        Handler {
            factory: self.factory.clone(),
            _t: PhantomData,
        }
    }
}

impl<T, P, R, O> Service for Handler<T, P, R, O> 
where 
    T: Factory<P, R, O>,
    R: Future<Output=O>,
    O: Responder,
{
    type Request = (P, ServiceRequest);
    type Response = ServiceResponse;
    type Error = ();
    type Future = HandlerServiceResponse<R, O>;

    fn call(&mut self, (param, _): (P, ServiceRequest)) -> Self::Future {
        HandlerServiceResponse {
            fut: self.factory.call(param),
            fut2: None,
        }
    }
}

pub struct HandlerServiceResponseProjection<'pin, R, O> 
where
    R: Future<Output = O>,
    O: Responder
{
    fut: Pin<&'pin mut R>,
    fut2: Pin<&'pin mut Option<O::Future>>,
}

pub struct HandlerServiceResponse<R, O> 
where
    R: Future<Output = O>,
    O: Responder
{
    fut: R,
    fut2: Option<O::Future>,
}

impl<R, O> HandlerServiceResponse<R, O> 
where
    R: Future<Output = O>,
    O: Responder
{
    fn _project<'pin>(self: Pin<&'pin mut Self>) -> HandlerServiceResponseProjection<'pin, R, O> {
        unsafe {
            let Self {fut, fut2} = self.get_unchecked_mut();
            HandlerServiceResponseProjection {
                fut: Pin::new_unchecked(fut),
                fut2: Pin::new_unchecked(fut2)
            }
        }
    }
}

impl<R, O> Future for HandlerServiceResponse<R, O> 
where 
    R: Future<Output = O>,
    O: Responder,
{
    type Output = Result<ServiceResponse, ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut()._project();
        if let Some(fut) = this.fut2.as_pin_mut() {
            return match fut.poll(cx) {
                Poll::Ready(res) => {
                    Poll::Ready(Ok(res))
                }
                Poll::Pending => Poll::Pending,
            };
        }
        match this.fut.poll(cx) {
            Poll::Ready(res) => {
                let fut = res.respond();
                self.as_mut()._project().fut2.set(Some(fut));
                self.poll(cx)
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
