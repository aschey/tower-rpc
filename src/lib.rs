use std::io;

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

#[derive(Clone, Copy)]
pub enum ServerMode {
    Multiplex,
    Pipeline,
}
