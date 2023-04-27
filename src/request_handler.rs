use async_trait::async_trait;

use futures::{future, Future, Stream};
use futures_cancel::FutureExt;
use pin_project::pin_project;

use std::{fmt::Debug, marker::PhantomData};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait RequestHandler: Send + Sync {
    type Req: Send + Sync;
    type Res: Send + Sync;

    async fn handle_request(
        &self,
        request: Self::Req,
        cancellation_token: CancellationToken,
    ) -> Self::Res;
}

pub fn handler_fn<F, Fut, Req, Res>(f: F) -> HandlerFn<F, Fut, Req, Res>
where
    Req: Send + Sync,
    Res: Send + Sync,
    Fut: Future<Output = Res> + Send + Sync,
    F: Fn(Req, CancellationToken) -> Fut + Send + Sync,
{
    HandlerFn {
        f,
        _phantom: Default::default(),
    }
}

pub fn channel<Req>() -> (
    impl RequestHandler<Req = Req, Res = ()>,
    mpsc::UnboundedReceiver<Req>,
)
where
    Req: Send + Sync + Debug + Clone + 'static,
{
    let (tx, rx) = mpsc::unbounded_channel();
    (
        handler_fn(move |req, _| {
            tx.send(req).unwrap();
            future::ready(())
        }),
        rx,
    )
}

#[derive(Clone)]
pub struct HandlerFn<F, Fut, Req, Res>
where
    Req: Send + Sync,
    Res: Send + Sync,
    Fut: Future<Output = Res> + Send + Sync,
    F: Fn(Req, CancellationToken) -> Fut + Send + Sync,
{
    f: F,
    _phantom: PhantomData<(Req, Res, Fut)>,
}

#[async_trait]
impl<F, Fut, Req, Res> RequestHandler for HandlerFn<F, Fut, Req, Res>
where
    Req: Send + Sync + Clone,
    Res: Send + Sync + Clone,
    Fut: Future<Output = Res> + Send + Sync,
    F: Fn(Req, CancellationToken) -> Fut + Send + Sync + Clone,
{
    type Req = Req;
    type Res = Res;

    async fn handle_request(
        &self,
        request: Self::Req,
        cancellation_token: CancellationToken,
    ) -> Self::Res {
        (self.f)(request, cancellation_token).await
    }
}

pub struct RequestHandlerStream<Req, Res> {
    _phantom: PhantomData<(Req, Res)>,
    request_tx: mpsc::UnboundedSender<(Req, Responder<Res>)>,
    request_rx: Option<mpsc::UnboundedReceiver<(Req, Responder<Res>)>>,
}

impl<Req, Res> Default for RequestHandlerStream<Req, Res> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Req, Res> RequestHandlerStream<Req, Res> {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        Self {
            _phantom: Default::default(),
            request_tx,
            request_rx: Some(request_rx),
        }
    }
    pub fn request_stream(&mut self) -> Option<RequestStream<Req, Res>> {
        self.request_rx
            .take()
            .map(|request_rx| RequestStream { request_rx })
    }
}

#[async_trait]
impl<Req, Res> RequestHandler for RequestHandlerStream<Req, Res>
where
    Req: Send + Sync + Debug,
    Res: Send + Sync + Debug,
{
    type Req = Req;
    type Res = Res;

    async fn handle_request(
        &self,
        request: Self::Req,
        cancellation_token: CancellationToken,
    ) -> Self::Res {
        let (tx, rx) = oneshot::channel();
        self.request_tx.send((request, Responder(tx))).unwrap();
        rx.cancel_on_shutdown(&cancellation_token)
            .await
            .unwrap()
            .unwrap()
    }
}

#[must_use = "Response must be sent"]
#[derive(Debug)]
pub struct Responder<Res>(oneshot::Sender<Res>);

impl<Res> Responder<Res> {
    pub fn respond(self, response: Res) -> Result<(), Res> {
        self.0.send(response)
    }
}

#[pin_project]
pub struct RequestStream<Req, Res> {
    #[pin]
    request_rx: mpsc::UnboundedReceiver<(Req, Responder<Res>)>,
}

impl<Req, Res> Stream for RequestStream<Req, Res> {
    type Item = (Req, Responder<Res>);

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.project().request_rx.poll_recv(cx)
    }
}
