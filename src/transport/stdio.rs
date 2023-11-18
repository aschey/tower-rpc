use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[pin_project]
pub struct StdioTransport<I, O> {
    #[pin]
    stdin: I,
    #[pin]
    stdout: O,
}

impl StdioTransport<tokio::io::Stdin, tokio::io::Stdout> {
    pub fn new() -> Self {
        Self {
            stdin: tokio::io::stdin(),
            stdout: tokio::io::stdout(),
        }
    }

    pub fn incoming() -> impl Stream<Item = Result<Self, io::Error>> {
        tokio_stream::once(Ok(Self::default()))
    }
}
impl Default for StdioTransport<tokio::io::Stdin, tokio::io::Stdout> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, O> StdioTransport<I, O> {
    pub fn attach(stdin: I, stdout: O) -> Self {
        Self { stdin, stdout }
    }
}

impl StdioTransport<tokio::process::ChildStdout, tokio::process::ChildStdin> {
    pub fn from_child(process: &mut tokio::process::Child) -> Option<Self> {
        Some(Self {
            stdin: process.stdout.take()?,
            stdout: process.stdin.take()?,
        })
    }
}

impl<I, O> AsyncRead for StdioTransport<I, O>
where
    I: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.project().stdin.poll_read(cx, buf)
    }
}

impl<I, O> AsyncWrite for StdioTransport<I, O>
where
    O: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().stdout.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().stdout.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().stdout.poll_flush(cx)
    }
}
