use core::fmt::Debug;
use futures::{Sink, Stream};
use std::{
    error::Error,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc::{self, error::SendError};

#[derive(Debug)]
pub struct LocalTransport<Req, Res> {
    tx: mpsc::UnboundedSender<Req>,
    rx: mpsc::UnboundedReceiver<Res>,
}

impl<Req: Debug, Res: Debug> Sink<Req> for LocalTransport<Req, Res> {
    type Error = Box<dyn Error + Send + Sync + 'static>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Req) -> Result<(), Self::Error> {
        self.tx.send(item).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl<Req, Res> Stream for LocalTransport<Req, Res> {
    type Item = Result<Res, Box<dyn Error + Send + Sync + 'static>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx).map(|s| s.map(Ok))
    }
}

pub struct LocalTransportFactory<Req, Res> {
    rx: mpsc::UnboundedReceiver<LocalTransport<Req, Res>>,
}

impl<Req, Res> Stream for LocalTransportFactory<Req, Res> {
    type Item = Result<LocalTransport<Req, Res>, Box<dyn Error + Send + Sync + 'static>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx).map(|s| s.map(Ok))
    }
}

pub fn unbounded<Req, Res>() -> (LocalTransportFactory<Req, Res>, LocalClientStream<Res, Req>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (LocalTransportFactory { rx }, LocalClientStream { tx })
}

pub struct LocalClientStream<Req, Res> {
    tx: mpsc::UnboundedSender<LocalTransport<Res, Req>>,
}

impl<Req: Debug, Res: Debug> LocalClientStream<Req, Res> {
    pub fn connect(&self) -> Result<LocalTransport<Req, Res>, SendError<LocalTransport<Res, Req>>> {
        let (tx1, rx2) = mpsc::unbounded_channel();
        let (tx2, rx1) = mpsc::unbounded_channel();
        let transport = LocalTransport::<Res, Req> { tx: tx1, rx: rx1 };
        self.tx.send(transport)?;
        Ok(LocalTransport { tx: tx2, rx: rx2 })
    }
}
