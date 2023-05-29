use bytes::{Bytes, BytesMut};
use futures::Stream;
use std::{error::Error, io, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_stream::StreamExt;
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

pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Send + Unpin + 'static {
    fn into_boxed(self) -> Box<dyn AsyncReadWrite>;
}

impl<T> AsyncReadWrite for T
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    fn into_boxed(self) -> Box<dyn AsyncReadWrite> {
        Box::new(self)
    }
}

pub trait IntoBoxedStream {
    fn into_boxed(
        self,
    ) -> Pin<
        Box<
            dyn Stream<Item = Result<Box<dyn AsyncReadWrite>, Box<dyn Error + Send + Sync>>> + Send,
        >,
    >;
}

trait IntoBoxedError {
    fn into_boxed(self) -> Box<dyn Error + Send + Sync + 'static>;
}

impl<E> IntoBoxedError for E
where
    E: Error + Send + Sync + Sized + 'static,
{
    fn into_boxed(self) -> Box<dyn Error + Send + Sync> {
        Box::new(self)
    }
}

impl<S, I, E> IntoBoxedStream for S
where
    S: Stream<Item = Result<I, E>> + Send + 'static,
    I: AsyncReadWrite,
    E: IntoBoxedError,
{
    fn into_boxed(
        self,
    ) -> Pin<
        Box<
            dyn Stream<Item = Result<Box<dyn AsyncReadWrite>, Box<dyn Error + Send + Sync>>> + Send,
        >,
    > {
        Box::pin(self.map(|i| i.map(|r| r.into_boxed()).map_err(|e| e.into_boxed())))
    }
}

pub trait IntoBoxedConnection {
    fn into_boxed(self) -> Box<dyn AsyncReadWrite + Send + Unpin>;
}

impl<S> IntoBoxedConnection for S
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    fn into_boxed(self) -> Box<dyn AsyncReadWrite + Send + Unpin> {
        Box::new(self)
    }
}
