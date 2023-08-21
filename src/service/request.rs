use std::marker::PhantomData;
use std::pin::Pin;
use std::task::Poll;

use background_service::ServiceContext;
use futures::Future;

use crate::Request;

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
