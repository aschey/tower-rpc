use std::convert::Infallible;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{future, Future, Stream};
use futures_cancel::FutureExt;
use pin_project::pin_project;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{self};
use tokio::sync::oneshot;
use tower::{BoxError, Service};

use crate::Request;

#[derive(Debug)]
pub struct MakeServiceFn<F, S, R>
where
    F: Fn() -> S,
    S: tower::Service<R>,
{
    f: F,
    _phantom: PhantomData<R>,
}

pub fn make_service_fn<F, S, R>(f: F) -> MakeServiceFn<F, S, R>
where
    F: Fn() -> S,
    S: Service<R>,
{
    MakeServiceFn {
        f,
        _phantom: Default::default(),
    }
}

impl<F, S, R> Service<()> for MakeServiceFn<F, S, R>
where
    F: Fn() -> S,
    S: Service<R>,
{
    type Error = Infallible;
    type Response = S;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        future::ready(Ok((self.f)()))
    }
}

pub fn channel<Req>() -> (ServiceChannel<Req>, mpsc::UnboundedReceiver<Request<Req>>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (ServiceChannel { tx }, rx)
}

pub struct ServiceChannel<Req> {
    tx: mpsc::UnboundedSender<Request<Req>>,
}

impl<Req> tower::Service<()> for ServiceChannel<Req> {
    type Error = BoxError;
    type Response = ServiceSender<Req>;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let sender = ServiceSender {
            tx: self.tx.clone(),
        };
        future::ready(Ok(sender))
    }
}

pub struct ServiceSender<Req> {
    tx: mpsc::UnboundedSender<Request<Req>>,
}

impl<Req: Debug> Service<Request<Req>> for ServiceSender<Req> {
    type Error = SendError<Request<Req>>;
    type Response = ();
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, req: Request<Req>) -> Self::Future {
        future::ready(self.tx.send(req))
    }
}

pub struct RequestHandlerStreamFactory<Req, Res> {
    _phantom: PhantomData<(Req, Res)>,
    request_tx: mpsc::UnboundedSender<(Request<Req>, Responder<Res>)>,
    request_rx: Option<mpsc::UnboundedReceiver<(Request<Req>, Responder<Res>)>>,
}

impl<Req, Res> Default for RequestHandlerStreamFactory<Req, Res> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Req, Res> RequestHandlerStreamFactory<Req, Res> {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        Self {
            _phantom: Default::default(),
            request_tx,
            request_rx: Some(request_rx),
        }
    }

    pub fn request_stream(&mut self) -> Option<RequestStream<Request<Req>, Res>> {
        self.request_rx
            .take()
            .map(|request_rx| RequestStream { request_rx })
    }
}

impl<Req, Res> tower::Service<()> for RequestHandlerStreamFactory<Req, Res> {
    type Error = BoxError;
    type Response = RequestHandlerStream<Req, Res>;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let stream = RequestHandlerStream {
            _phantom: Default::default(),
            request_tx: self.request_tx.clone(),
        };
        future::ready(Ok(stream))
    }
}

pub struct RequestHandlerStream<Req, Res> {
    _phantom: PhantomData<(Req, Res)>,
    request_tx: mpsc::UnboundedSender<(Request<Req>, Responder<Res>)>,
}

impl<Req, Res> Service<Request<Req>> for RequestHandlerStream<Req, Res>
where
    Req: Send + Sync + Debug + 'static,
    Res: Send + Sync + Debug + 'static,
{
    type Error = BoxError;
    type Response = Res;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, req: Request<Req>) -> Self::Future {
        let (tx, rx) = oneshot::channel();
        let cancellation_token = req.context.cancellation_token();
        let res = self.request_tx.send((req, Responder(tx)));
        Box::pin(async move {
            res?;
            Ok(rx.cancel_on_shutdown(&cancellation_token).await??)
        })
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

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().request_rx.poll_recv(cx)
    }
}

pub trait MakeHandler<Req>
where
    Self: tower::Service<Req> + Default + Sized,
{
    fn make(_: ()) -> impl Future<Output = Result<Self, Infallible>> + Send;
}

impl<S, Req> MakeHandler<Req> for S
where
    S: tower::Service<Req> + Default,
{
    async fn make(_: ()) -> Result<Self, Infallible> {
        Ok(Self::default())
    }
}
