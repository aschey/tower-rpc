use std::{
    collections::VecDeque,
    fmt::Debug,
    future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use async_trait::async_trait;
use background_service::ServiceContext;
use futures::Future;
use matchit::{InsertError, MatchError, Params, Router};
use tower::{BoxError, Service, ServiceExt};

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
    pub fn with_route(mut self, route: impl Into<String>, service: S) -> Result<Self, InsertError> {
        self.router.insert(route, self.route_index)?;
        self.services.push(service);
        self.not_ready.push_back(self.route_index);
        self.route_index += 1;
        Ok(self)
    }
}

impl<Req, S> Service<Request<RoutedRequest<Req>>> for RouteService<Req, S>
where
    S: Service<RouteMatch<Req>>,
    S::Error: Debug,
    S::Future: Send + 'static,
    S::Response: Send + 'static,
{
    type Error = BoxError;
    type Response = S::Response;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            // must wait for *all* services to be ready.
            // this will cause head-of-line blocking unless the underlying services are always ready.
            if self.not_ready.is_empty() {
                return Poll::Ready(Ok(()));
            } else {
                if self.services[self.not_ready[0]]
                    .poll_ready(cx)
                    .map_err(|e| format!("{e:?}"))?
                    .is_pending()
                {
                    return Poll::Pending;
                }

                self.not_ready.pop_front();
            }
        }
    }

    fn call(&mut self, req: Request<RoutedRequest<Req>>) -> Self::Future {
        let svc_index = match self.router.at(&req.value.route) {
            Ok(index) => index,
            Err(e) => {
                return Box::pin(future::ready(Err(e.into())));
            }
        };

        self.not_ready.push_back(*svc_index.value);
        let inner_rs = self.services[*svc_index.value].call(RouteMatch {
            context: req.context,
            route: req.value.route,
            router: self.router.clone(),
            value: req.value.value,
        });
        Box::pin(async move {
            let res = inner_rs.await;
            Ok(res.map_err(|e| format!("{e:?}"))?)
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
    pub fn params(&self) -> Result<Params, MatchError> {
        Ok(self.router.at(&self.route)?.params)
    }
}

#[async_trait]
pub trait CallRoute<Request>: Service<RoutedRequest<Request>> {
    fn call_route(&mut self, route: impl Into<String>, request: Request) -> Self::Future;

    async fn call_route_ready(
        &mut self,
        route: impl Into<String> + Send,
        request: Request,
    ) -> Result<Self::Response, Self::Error>;
}

#[async_trait]
impl<Request, S> CallRoute<Request> for S
where
    Request: Send + 'static,
    S::Future: Send,
    S::Error: Send,
    S: Service<RoutedRequest<Request>> + Send,
{
    fn call_route(&mut self, route: impl Into<String>, request: Request) -> Self::Future {
        self.call(RoutedRequest {
            route: route.into(),
            value: request,
        })
    }

    async fn call_route_ready(
        &mut self,
        route: impl Into<String> + Send,
        request: Request,
    ) -> Result<Self::Response, Self::Error> {
        self.ready().await?;
        self.call_route(route, request).await
    }
}
