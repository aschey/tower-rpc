use core::fmt::Debug;
use futures::{Sink, Stream};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc;

pub struct LocalTransport<T> {
    tx: mpsc::UnboundedSender<T>,
    rx: mpsc::UnboundedReceiver<T>,
}

impl<T> Default for LocalTransport<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LocalTransport<T> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx }
    }
}

impl<T: Debug> Sink<T> for LocalTransport<T> {
    type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.tx.send(item).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl<T> Stream for LocalTransport<T> {
    type Item = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx).map(|s| s.map(Ok))
    }
}
