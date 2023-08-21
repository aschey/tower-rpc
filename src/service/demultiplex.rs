use std::marker::PhantomData;
use std::pin::Pin;
use std::task::Poll;

use futures::Future;

#[derive(Clone, Debug)]
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
