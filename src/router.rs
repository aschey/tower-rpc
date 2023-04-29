use std::{
    collections::VecDeque,
    marker::PhantomData,
    task::{Context, Poll},
};

use background_service::ServiceContext;
use matchit::{Params, Router};
use tower::Service;

use crate::Request;

pub struct RouteService<Req, S>
where
    S: Service<RouteMatch<Req>>,
{
    services: Vec<S>,
    router: Router<usize>,
    route_index: usize,
    not_ready: VecDeque<usize>,
    _phantom: PhantomData<Req>,
}

impl<Req, S> Default for RouteService<Req, S>
where
    S: Service<RouteMatch<Req>>,
{
    fn default() -> Self {
        Self {
            services: Default::default(),
            router: Default::default(),
            route_index: 0,
            not_ready: Default::default(),
            _phantom: Default::default(),
        }
    }
}

impl<Req, S> RouteService<Req, S>
where
    S: Service<RouteMatch<Req>>,
{
    pub fn with_route(mut self, route: impl Into<String>, service: S) -> Self {
        self.router.insert(route, self.route_index).unwrap();
        self.services.push(service);
        self.not_ready.push_back(self.route_index);
        self.route_index += 1;
        self
    }
}

impl<Req, S> Service<Request<RoutedRequest<Req>>> for RouteService<Req, S>
where
    S: Service<RouteMatch<Req>>,
{
    type Error = S::Error;
    type Response = S::Response;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            // must wait for *all* services to be ready.
            // this will cause head-of-line blocking unless the underlying services are always ready.
            if self.not_ready.is_empty() {
                return Poll::Ready(Ok(()));
            } else {
                if self.services[self.not_ready[0]]
                    .poll_ready(cx)?
                    .is_pending()
                {
                    return Poll::Pending;
                }

                self.not_ready.pop_front();
            }
        }
    }

    fn call(&mut self, req: Request<RoutedRequest<Req>>) -> Self::Future {
        let svc_index = self.router.at(&req.value.route).unwrap();

        self.not_ready.push_back(*svc_index.value);
        self.services[*svc_index.value].call(RouteMatch {
            context: req.context,
            route: req.value.route,
            router: self.router.clone(),
            value: req.value.value,
        })
    }
}

#[derive(Debug)]
pub struct RoutedRequest<T> {
    pub route: String,
    pub value: T,
}

pub struct RouteMatch<T> {
    pub context: ServiceContext,
    pub route: String,
    pub value: T,
    router: Router<usize>,
}

impl<T> RouteMatch<T> {
    pub fn params(&self) -> Params {
        self.router.at(&self.route).unwrap().params
    }
}

pub trait CallRoute<Request> {
    type Future;
    fn call_route(&mut self, route: impl Into<String>, request: Request) -> Self::Future;
}

impl<Request, T> CallRoute<Request> for T
where
    T: Service<RoutedRequest<Request>>,
{
    type Future = <Self as Service<RoutedRequest<Request>>>::Future;

    fn call_route(&mut self, route: impl Into<String>, request: Request) -> Self::Future {
        self.call(RoutedRequest {
            route: route.into(),
            value: request,
        })
    }
}
