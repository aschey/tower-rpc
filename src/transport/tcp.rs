use futures::Stream;
use pin_project::pin_project;
use std::{net::SocketAddr, task::Poll};
use tokio::{io, net::TcpListener};

pub type TcpStream = tokio::net::TcpStream;

pub async fn create_endpoint(addr: SocketAddr) -> io::Result<TcpTransport> {
    TcpTransport::bind(addr).await
}

#[pin_project]
pub struct TcpTransport {
    #[pin]
    listener: TcpListener,
}

impl TcpTransport {
    async fn bind(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
        })
    }
}

impl Stream for TcpTransport {
    type Item = Result<TcpStream, io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.project().listener.poll_accept(cx) {
            Poll::Ready(Ok((stream, _))) => Poll::Ready(Some(Ok(stream))),
            Poll::Ready(Err(_)) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
