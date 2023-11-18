use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{CodecBuilder, CodecStream};

#[pin_project]
pub struct CodecTransport<B, I, E, S>
where
    B: CodecBuilder,
    S: Stream<Item = Result<I, E>>,
    I: AsyncRead + AsyncWrite,
{
    #[pin]
    codec_builder: B,
    #[pin]
    inner: S,
}

impl<B, I, E, S> CodecTransport<B, I, E, S>
where
    B: CodecBuilder,
    S: Stream<Item = Result<I, E>>,
    I: AsyncRead + AsyncWrite,
{
    pub fn new(inner: S, codec_builder: B) -> Self {
        Self {
            inner,
            codec_builder,
        }
    }
}

impl<B, I, E, S> Stream for CodecTransport<B, I, E, S>
where
    B: CodecBuilder,
    S: Stream<Item = Result<I, E>>,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Item = Result<CodecStream<B::Req, B::Res, B::StreamErr, B::SinkErr>, E>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(stream))) => {
                Poll::Ready(Some(Ok(this.codec_builder.build_codec(Box::new(stream)))))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
