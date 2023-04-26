use std::io;

use futures::Stream;
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};

#[pin_project]
pub struct StdioTransport {
    #[pin]
    stdin: tokio::io::Stdin,
    #[pin]
    stdout: tokio::io::Stdout,
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            stdin: tokio::io::stdin(),
            stdout: tokio::io::stdout(),
        }
    }

    pub fn incoming() -> impl Stream<Item = Result<StdioTransport, io::Error>> {
        tokio_stream::once(Ok(Self::default()))
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncRead for StdioTransport {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().stdin.poll_read(cx, buf)
    }
}

impl AsyncWrite for StdioTransport {
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
