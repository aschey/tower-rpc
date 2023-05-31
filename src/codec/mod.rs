use bytes::{Bytes, BytesMut};
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::LengthDelimitedCodec;

mod builder;
pub use builder::*;
#[cfg(feature = "serde-codec")]
mod serde;
#[cfg(feature = "serde-codec")]
pub use self::serde::*;
#[cfg(feature = "serde-codec")]
mod serializer;
#[cfg(feature = "serde-codec")]
pub use serializer::*;

#[derive(Clone, Debug, Copy)]
pub enum Codec {
    #[cfg(feature = "bincode")]
    Bincode,
    #[cfg(feature = "json")]
    Json,
    #[cfg(feature = "messagepack")]
    MessagePack,
    #[cfg(feature = "cbor")]
    Cbor,
}

pub fn length_delimited_codec(
    incoming: impl AsyncReadWrite,
) -> CodecStream<BytesMut, Bytes, io::Error, io::Error> {
    Box::new(tokio_util::codec::Framed::new(
        incoming,
        LengthDelimitedCodec::new(),
    ))
}

pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Send + Unpin + 'static {}

impl<T> AsyncReadWrite for T where T: AsyncRead + AsyncWrite + Send + Unpin + 'static {}
