use std::{io, marker::PhantomData};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::LengthDelimitedCodec;

use crate::{AsyncReadWrite, Codec, CodecBuilder, CodecStream};

use super::serializer::CodecSerializer;

pub fn serde_codec<Req, Res>(
    incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
    codec: Codec,
) -> CodecStream<Req, Res, io::Error, io::Error>
where
    Req: serde::Serialize + for<'de> serde::Deserialize<'de> + Unpin + Send + 'static,
    Res: serde::Serialize + for<'de> serde::Deserialize<'de> + Unpin + Send + 'static,
{
    let stream = tokio_util::codec::Framed::new(incoming, LengthDelimitedCodec::new());

    let stream = tokio_serde::Framed::new(stream, CodecSerializer::new(codec));
    Box::new(stream)
}

pub struct SerdeCodec<Req, Res> {
    _phantom: PhantomData<(Req, Res)>,
    codec: crate::Codec,
}

impl<Req, Res> SerdeCodec<Req, Res> {
    pub fn new(codec: crate::Codec) -> Self {
        Self {
            codec,
            _phantom: Default::default(),
        }
    }
}

impl<Req, Res> CodecBuilder for SerdeCodec<Req, Res>
where
    Req: serde::Serialize + for<'de> serde::Deserialize<'de> + Unpin + Send + 'static,
    Res: serde::Serialize + for<'de> serde::Deserialize<'de> + Unpin + Send + 'static,
{
    type Req = Req;
    type Res = Res;
    type SinkErr = io::Error;
    type StreamErr = io::Error;

    fn build_codec(
        &self,
        incoming: Box<dyn AsyncReadWrite>,
    ) -> CodecStream<Self::Req, Self::Res, Self::StreamErr, Self::SinkErr> {
        serde_codec(incoming, self.codec)
    }
}
