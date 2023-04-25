use futures::Future;
use std::{io, pin::Pin, sync::Arc, task::Poll};
use tokio_util::sync::CancellationToken;

use crate::RequestHandler;

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
