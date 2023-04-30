use std::io;

use futures::Stream;
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};

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
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().stdin.poll_read(cx, buf)
    }
}

impl<I, O> AsyncWrite for StdioTransport<I, O>
where
    O: AsyncWrite,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.project().stdout.poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.project().stdout.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.project().stdout.poll_flush(cx)
    }
}
