use std::{net::SocketAddr, task::Poll};

use futures::Stream;
use pin_project::pin_project;
use tokio::net::{TcpListener, TcpStream};

#[pin_project]
pub struct TcpTransport {
    #[pin]
    listener: TcpListener,
}

impl TcpTransport {
    pub async fn bind(addr: SocketAddr) -> Self {
        Self {
            listener: TcpListener::bind(addr).await.unwrap(),
        }
    }
}

impl Stream for TcpTransport {
    type Item = Result<TcpStream, Box<dyn std::error::Error + Send + Sync>>;

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
