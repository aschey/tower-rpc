use futures::Future;
use std::{io, marker::PhantomData, pin::Pin, sync::Arc, task::Poll};
use tokio_util::sync::CancellationToken;

use crate::{RequestHandler, Tagged};

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

#[derive(Default)]
pub struct RpcService<H>
where
    H: RequestHandler + 'static,
{
    handler: Arc<H>,
    cancellation_token: CancellationToken,
}

impl<H> RpcService<H>
where
    H: RequestHandler + 'static,
{
    pub fn new(handler: Arc<H>, cancellation_token: CancellationToken) -> Self {
        Self {
            handler,
            cancellation_token,
        }
    }
}

impl<H> tower::Service<H::Req> for RpcService<H>
where
    H: RequestHandler + 'static,
{
    type Response = H::Res;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: H::Req) -> Self::Future {
        let cancellation_token = self.cancellation_token.clone();
        let handler = self.handler.clone();

        Box::pin(async move { Ok(handler.handle_request(req, cancellation_token).await) })
    }
}
