use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::future;
use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use background_service::ServiceContext;
use futures::Future;
use matchit::{InsertError, MatchError, Params, Router};
use tower::{BoxError, Service, ServiceExt};

use crate::Request;

pub trait RouteKey {
    type Key;
}

#[derive(Debug)]
pub struct Unkeyed;
impl RouteKey for Unkeyed {
    type Key = ();
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Keyed<T>(pub T);

impl<T> RouteKey for Keyed<T> {
    type Key = T;
}

#[cfg(feature = "serde-codec")]
pub fn routed_codec<Req, Res>(
    codec: crate::Codec,
) -> crate::SerdeCodec<RoutedRequest<Req, Unkeyed>, Res> {
    crate::SerdeCodec::<RoutedRequest<Req, Unkeyed>, Res>::new(codec)
}

#[cfg(feature = "serde-codec")]
pub fn keyed_codec<Req, Res, K>(
    codec: crate::Codec,
) -> crate::SerdeCodec<RoutedRequest<Req, Keyed<K>>, Res> {
    crate::SerdeCodec::<RoutedRequest<Req, Keyed<K>>, Res>::new(codec)
}

#[derive(Clone)]
pub struct RouteService<Req, S, K = Unkeyed>
where
    K: RouteKey,
{
    services: Vec<S>,
    routers: HashMap<K::Key, Router<usize>>,
    route_index: usize,
    not_ready: VecDeque<usize>,
    _phantom: PhantomData<Req>,
}

impl<Req, S> Default for RouteService<Req, S>
where
    S: Service<RouteMatch<Req, Unkeyed>>,
{
    fn default() -> Self {
        let mut routers = HashMap::<_, _>::default();
        routers.insert((), Default::default());
        Self {
            services: Default::default(),
            routers,
            route_index: 0,
            not_ready: Default::default(),
            _phantom: Default::default(),
        }
    }
}

impl<Req, S> RouteService<Req, S> {
    pub fn with_route(self, route: impl Into<String>, service: S) -> Self {
        self.try_with_route(route, service).expect("invalid route")
    }

    pub fn try_with_route(
        mut self,
        route: impl Into<String>,
        service: S,
    ) -> Result<Self, InsertError> {
        self.routers
            .get_mut(&())
            .expect("type error")
            .insert(route, self.route_index)?;
        self.services.push(service);
        self.not_ready.push_back(self.route_index);
        self.route_index += 1;
        Ok(self)
    }
}

impl<Req, S, K> RouteService<Req, S, Keyed<K>>
where
    K: Hash + Eq + PartialEq,
{
    pub fn with_keys() -> Self {
        let routers = HashMap::<_, _>::default();

        Self {
            services: Default::default(),
            routers,
            route_index: 0,
            not_ready: Default::default(),
            _phantom: Default::default(),
        }
    }

    pub fn with_route(self, key: impl Into<K>, route: impl Into<String>, service: S) -> Self {
        self.try_with_route(key, route, service)
            .expect("invalid route")
    }

    pub fn try_with_route(
        mut self,
        key: impl Into<K>,
        route: impl Into<String>,
        service: S,
    ) -> Result<Self, InsertError> {
        let key = key.into();
        match self.routers.get_mut(&key) {
            Some(router) => {
                router.insert(route, self.route_index)?;
            }
            None => {
                let mut router = Router::default();
                router.insert(route, self.route_index)?;
                self.routers.insert(key, router);
            }
        }
        self.services.push(service);
        self.not_ready.push_back(self.route_index);
        self.route_index += 1;
        Ok(self)
    }
}

impl<Req, S, K> Service<Request<RoutedRequest<Req, K>>> for RouteService<Req, S, K>
where
    S: Service<RouteMatch<Req, K>>,
    S::Error: Debug,
    S::Future: Send + 'static,
    S::Response: Send + 'static,
    K: RouteKey,
    K::Key: Hash + PartialEq + Eq,
{
    type Error = BoxError;
    type Response = S::Response;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            // must wait for *all* services to be ready.
            // this will cause head-of-line blocking unless the underlying services are always
            // ready.
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

    fn call(&mut self, req: Request<RoutedRequest<Req, K>>) -> Self::Future {
        let router = match self.routers.get(&req.value.key) {
            Some(router) => router,
            None => {
                return Box::pin(future::ready(Err("No router found".into())));
            }
        };
        let svc_index = match router.at(&req.value.route) {
            Ok(index) => index,
            Err(e) => {
                return Box::pin(future::ready(Err(e.into())));
            }
        };

        self.not_ready.push_back(*svc_index.value);
        let inner_rs = self.services[*svc_index.value].call(RouteMatch {
            context: req.context,
            route: req.value.route,
            key: req.value.key,
            router: router.clone(),
            value: req.value.value,
        });
        Box::pin(async move {
            let res = inner_rs.await;
            Ok(res.map_err(|e| format!("{e:?}"))?)
        })
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde-codec", derive(serde::Serialize, serde::Deserialize))]
pub struct RoutedRequest<T, K: RouteKey> {
    pub route: String,
    pub key: K::Key,
    pub value: T,
}

pub struct RouteMatch<T, K: RouteKey = Unkeyed> {
    pub context: ServiceContext,
    pub route: String,
    pub key: K::Key,
    pub value: T,
    router: Router<usize>,
}

impl<T, K: RouteKey> RouteMatch<T, K> {
    pub fn params(&self) -> Result<Params, MatchError> {
        Ok(self.router.at(&self.route)?.params)
    }
}

#[async_trait]
pub trait CallRoute<Request>: Service<RoutedRequest<Request, Unkeyed>> {
    fn call_route(&mut self, route: impl Into<String>, request: Request) -> Self::Future;

    async fn call_route_ready(
        &mut self,
        route: impl Into<String> + Send,
        request: Request,
    ) -> Result<Self::Response, Self::Error>;
}

#[async_trait]
pub trait CallKeyedRoute<Request, K>: Service<RoutedRequest<Request, Keyed<K>>> {
    fn call_route(
        &mut self,
        key: impl Into<K>,
        route: impl Into<String>,
        request: Request,
    ) -> Self::Future;

    async fn call_route_ready(
        &mut self,
        key: impl Into<K> + Send,
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
    S: Service<RoutedRequest<Request, Unkeyed>> + Send,
{
    fn call_route(&mut self, route: impl Into<String>, request: Request) -> Self::Future {
        self.call(RoutedRequest {
            route: route.into(),
            key: (),
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

#[async_trait]
impl<Request, S, K> CallKeyedRoute<Request, K> for S
where
    Request: Send + 'static,
    S::Future: Send,
    S::Error: Send,
    S: Service<RoutedRequest<Request, Keyed<K>>> + Send,
    K: Send + 'static,
{
    fn call_route(
        &mut self,
        key: impl Into<K>,
        route: impl Into<String>,
        request: Request,
    ) -> Self::Future {
        self.call(RoutedRequest {
            route: route.into(),
            key: key.into(),
            value: request,
        })
    }

    async fn call_route_ready(
        &mut self,
        key: impl Into<K> + Send,
        route: impl Into<String> + Send,
        request: Request,
    ) -> Result<Self::Response, Self::Error> {
        self.ready().await?;
        self.call_route(key, route, request).await
    }
}
