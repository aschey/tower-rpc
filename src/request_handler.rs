use std::{error::Error, fmt::Debug, io, marker::PhantomData};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures::{Future, Sink, Stream};
use futures_cancel::FutureExt;
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot},
};
use tokio_util::sync::CancellationToken;

use crate::Codec;

pub trait StreamSink<SinkItem>: Stream + Sink<SinkItem> + Unpin + Send {}

impl<T, SinkItem> StreamSink<SinkItem> for T where T: Stream + Sink<SinkItem> + Unpin + Send {}

pub type CodecStream<Req, Res, StreamErr, SinkErr> =
    Box<dyn StreamSink<Res, Item = Result<Req, StreamErr>, Error = SinkErr>>;

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

impl<Res> Responder<Res>
where
    Res: Default,
{
    pub fn ack(self) -> Result<(), Res> {
        self.0.send(Res::default())
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

pub trait CodecBuilder: Send {
    type Req: Send;
    type Res: Send;
    type StreamErr: Error + Send + Sync + 'static;
    type SinkErr: Error + Send + Sync + 'static;

    fn build_codec(
        &self,
        incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    ) -> CodecStream<Self::Req, Self::Res, Self::StreamErr, Self::SinkErr>;
}

pub struct SerdeCodec<Req, Res> {
    _phantom: PhantomData<(Req, Res)>,
    codec: Codec,
}

impl<Req, Res> SerdeCodec<Req, Res> {
    pub fn new(codec: Codec) -> Self {
        Self {
            codec,
            _phantom: Default::default(),
        }
    }
}

impl<Req, Res> CodecBuilder for SerdeCodec<Req, Res>
where
    Req: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
    Res: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
{
    type Req = Req;
    type Res = Res;
    type SinkErr = io::Error;
    type StreamErr = io::Error;

    fn build_codec(
        &self,
        incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    ) -> CodecStream<Self::Req, Self::Res, Self::StreamErr, Self::SinkErr> {
        crate::serde_codec(incoming, self.codec)
    }
}

pub struct LengthDelimitedCodec;

impl CodecBuilder for LengthDelimitedCodec {
    type Req = BytesMut;
    type Res = Bytes;
    type SinkErr = io::Error;
    type StreamErr = io::Error;

    fn build_codec(
        &self,
        incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    ) -> CodecStream<Self::Req, Self::Res, Self::StreamErr, Self::SinkErr> {
        crate::length_delimited_codec(incoming)
    }
}
