use self::serializer::CodecSerializer;
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::LengthDelimitedCodec;

mod builder;
pub use builder::*;
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

pub fn serde_codec<Req, Res>(
    incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    codec: Codec,
) -> CodecStream<Req, Res, io::Error, io::Error>
where
    Req: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
    Res: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
{
    let stream = tokio_util::codec::Framed::new(incoming, LengthDelimitedCodec::new());

    let stream = tokio_serde::Framed::new(stream, CodecSerializer::new(codec));
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
