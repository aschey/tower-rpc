use std::{error::Error, io, marker::PhantomData};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures::{Future, Sink, Stream};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::sync::CancellationToken;

pub trait StreamSink<SinkItem>: Stream + Sink<SinkItem> + Unpin + Send {}

impl<T, SinkItem> StreamSink<SinkItem> for T where T: Stream + Sink<SinkItem> + Unpin + Send {}

pub type Transport<Req, Res, StreamErr, SinkErr> =
    Box<dyn StreamSink<Res, Item = Result<Req, StreamErr>, Error = SinkErr>>;

#[async_trait]
pub trait IpcRequestHandler: Send + Sync {
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
impl<F, Fut, Req, Res> IpcRequestHandler for HandlerFn<F, Fut, Req, Res>
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

pub trait TransportBuilder: Send {
    type Req: Send;
    type Res: Send;
    type StreamErr: Error + Send + Sync + 'static;
    type SinkErr: Error + Send + Sync + 'static;

    fn build_transport(
        &self,
        incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    ) -> Transport<Self::Req, Self::Res, Self::StreamErr, Self::SinkErr>;
}

#[derive(Default)]
pub struct CodecTransport<Req, Res> {
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req, Res> TransportBuilder for CodecTransport<Req, Res>
where
    Req: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
    Res: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
{
    type Req = Req;
    type Res = Res;
    type SinkErr = io::Error;
    type StreamErr = io::Error;

    fn build_transport(
        &self,
        incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    ) -> Transport<Self::Req, Self::Res, Self::StreamErr, Self::SinkErr> {
        crate::codec_transport(incoming)
    }
}

pub struct LengthDelimitedTransport;

impl TransportBuilder for LengthDelimitedTransport {
    type Req = BytesMut;
    type Res = Bytes;
    type SinkErr = io::Error;
    type StreamErr = io::Error;

    fn build_transport(
        &self,
        incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    ) -> Transport<Self::Req, Self::Res, Self::StreamErr, Self::SinkErr> {
        crate::length_delimited_transport(incoming)
    }
}
