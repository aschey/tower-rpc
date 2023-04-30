use std::io;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::LengthDelimitedCodec;

mod codec;
pub use codec::*;
mod codec_wrapper;
pub use codec_wrapper::*;
mod request_handler;
pub use request_handler::*;
mod rpc_service;
mod server;
pub use server::*;
mod client;
pub use client::*;
mod tagged;
pub use tagged::*;
mod codec_builder;
pub mod transport;
pub use codec_builder::*;
mod request;
pub use request::*;
mod router;
pub use router::*;
use tower::{Service, ServiceExt};

pub fn serde_codec<Req, Res>(
    incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    codec: Codec,
) -> CodecStream<Req, Res, io::Error, io::Error>
where
    Req: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
    Res: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
{
    let stream = tokio_util::codec::Framed::new(incoming, LengthDelimitedCodec::new());

    let stream = tokio_serde::Framed::new(stream, CodecWrapper::new(codec));
    Box::new(stream)
}

pub fn length_delimited_codec(
    incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
) -> CodecStream<BytesMut, Bytes, io::Error, io::Error> {
    Box::new(tokio_util::codec::Framed::new(
        incoming,
        LengthDelimitedCodec::new(),
    ))
}

mod private {
    pub trait Sealed {}
}

pub trait ServerMode: private::Sealed {}

pub struct Pipeline;
impl private::Sealed for Pipeline {}
impl ServerMode for Pipeline {}

pub struct Multiplex {}
impl private::Sealed for Multiplex {}
impl ServerMode for Multiplex {}

#[async_trait]
pub trait ReadyServiceExt<Request>: Service<Request>
where
    Request: Send,
{
    async fn call_ready(&mut self, request: Request) -> Result<Self::Response, Self::Error>;
}

#[async_trait]
impl<S, Request> ReadyServiceExt<Request> for S
where
    Request: Send + 'static,
    S::Future: Send,
    S::Error: Send,
    S: Service<Request> + Send,
{
    async fn call_ready(&mut self, request: Request) -> Result<Self::Response, Self::Error> {
        self.ready().await?.call(request).await
    }
}
