use background_service::ServiceContext;
use futures::Future;
use std::{marker::PhantomData, pin::Pin, task::Poll};

use crate::{Request, Tagged};

pub struct MultiplexService<S> {
    inner: S,
}

impl<S> MultiplexService<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Req> tower::Service<Tagged<Req>> for MultiplexService<S>
where
    Req: Send,
    S: tower::Service<Req>,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Response = Tagged<S::Response>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Tagged<Req>) -> Self::Future {
        let res = self.inner.call(req.value);
        Box::pin(async move {
            let value = res.await?;
            Ok(Tagged {
                value,
                tag: req.tag,
            })
        })
    }
}

pub struct DemultiplexService<S, Res> {
    inner: S,
    _phantom: PhantomData<Res>,
}

impl<S, Res> DemultiplexService<S, Res> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            _phantom: Default::default(),
        }
    }
}

impl<S, Req, Res> tower::Service<Req> for DemultiplexService<S, Res>
where
    Req: Send,
    S: tower::Service<Tagged<Req>, Response = Tagged<Res>>,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Response = Res;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let res = self.inner.call(Tagged { tag: 0, value: req });
        Box::pin(async move {
            let res = res.await?;
            Ok(res.value)
        })
    }
}

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
