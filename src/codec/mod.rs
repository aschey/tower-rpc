use bytes::{Bytes, BytesMut};
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::LengthDelimitedCodec;

mod builder;
pub use builder::*;
#[cfg(feature = "serde-codec")]
mod serializer;

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

#[cfg(feature = "serde-codec")]
pub fn serde_codec<Req, Res>(
    incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    codec: Codec,
) -> CodecStream<Req, Res, io::Error, io::Error>
where
    Req: serde::Serialize + for<'de> serde::Deserialize<'de> + Unpin + Send + 'static,
    Res: serde::Serialize + for<'de> serde::Deserialize<'de> + Unpin + Send + 'static,
{
    let stream = tokio_util::codec::Framed::new(incoming, LengthDelimitedCodec::new());

    let stream = tokio_serde::Framed::new(stream, self::serializer::CodecSerializer::new(codec));
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
