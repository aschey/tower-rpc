use background_service::ServiceContext;
use futures::Future;
use std::{marker::PhantomData, pin::Pin, task::Poll};

use crate::Request;

#[cfg(feature = "multiplex")]
#[derive(Clone, Debug)]
pub struct MultiplexService<S> {
    inner: S,
}

#[cfg(feature = "multiplex")]
impl<S> MultiplexService<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "multiplex")]
impl<S, Req> tower::Service<crate::Tagged<Req>> for MultiplexService<S>
where
    Req: Send,
    S: tower::Service<Req>,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Response = crate::Tagged<S::Response>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: crate::Tagged<Req>) -> Self::Future {
        let res = self.inner.call(req.value);
        Box::pin(async move {
            let value = res.await?;
            Ok(crate::Tagged {
                value,
                tag: req.tag,
            })
        })
    }
}

#[cfg(feature = "multiplex")]
#[derive(Clone, Debug)]
pub struct DemultiplexService<S, Res> {
    inner: S,
    _phantom: PhantomData<Res>,
}

#[cfg(feature = "multiplex")]
impl<S, Res> DemultiplexService<S, Res> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            _phantom: Default::default(),
        }
    }
}

#[cfg(feature = "multiplex")]
impl<S, Req, Res> tower::Service<Req> for DemultiplexService<S, Res>
where
    Req: Send,
    S: tower::Service<crate::Tagged<Req>, Response = crate::Tagged<Res>>,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Response = Res;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let res = self.inner.call(crate::Tagged { tag: 0, value: req });
        Box::pin(async move {
            let res = res.await?;
            Ok(res.value)
        })
    }
}

#[derive(Clone)]
pub struct RequestService<S, Res> {
    context: ServiceContext,
    inner: S,
    _phantom: PhantomData<Res>,
}

impl<S, Res> RequestService<S, Res> {
    pub fn new(context: ServiceContext, inner: S) -> Self {
        Self {
            context,
            inner,
            _phantom: Default::default(),
        }
    }
}

impl<S, Req, Res> tower::Service<Req> for RequestService<S, Res>
where
    Req: Send,
    S: tower::Service<Request<Req>, Response = Res>,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Response = Res;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let res = self.inner.call(Request {
            context: self.context.clone(),
            value: req,
        });
        Box::pin(async move {
            let res = res.await?;
            Ok(res)
        })
    }
}
