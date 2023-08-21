use core::fmt::Debug;
use std::error::Error;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Sink, Stream};
use tokio::sync::mpsc::{self};

#[derive(Debug)]
enum Sender<T> {
    Bounded(mpsc::Sender<T>),
    Unbounded(mpsc::UnboundedSender<T>),
}

#[derive(Debug)]
enum Receiver<T> {
    Bounded(mpsc::Receiver<T>),
    Unbounded(mpsc::UnboundedReceiver<T>),
}

#[derive(Debug)]
pub struct LocalTransport<Req, Res> {
    tx: Sender<Req>,
    rx: Receiver<Res>,
}

impl<Req: Debug, Res: Debug> Sink<Req> for LocalTransport<Req, Res> {
    type Error = Box<dyn Error + Send + Sync + 'static>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Req) -> Result<(), Self::Error> {
        match &self.tx {
            Sender::Bounded(tx) => tx.try_send(item).map_err(|e| e.to_string())?,
            Sender::Unbounded(tx) => tx.send(item).map_err(|e| e.to_string())?,
        }
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
        match &mut self.rx {
            Receiver::Bounded(rx) => rx.poll_recv(cx).map(|s| s.map(Ok)),
            Receiver::Unbounded(rx) => rx.poll_recv(cx).map(|s| s.map(Ok)),
        }
    }
}

pub struct LocalTransportFactory<Req, Res> {
    rx: Receiver<LocalTransport<Req, Res>>,
}

impl<Req, Res> Stream for LocalTransportFactory<Req, Res> {
    type Item = Result<LocalTransport<Req, Res>, Box<dyn Error + Send + Sync + 'static>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.rx {
            Receiver::Bounded(rx) => rx.poll_recv(cx).map(|s| s.map(Ok)),
            Receiver::Unbounded(rx) => rx.poll_recv(cx).map(|s| s.map(Ok)),
        }
    }
}

pub fn unbounded_channel<Req, Res>()
-> (LocalTransportFactory<Req, Res>, LocalClientStream<Res, Req>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        LocalTransportFactory {
            rx: Receiver::Unbounded(rx),
        },
        LocalClientStream {
            tx: Sender::Unbounded(tx),
        },
    )
}

pub fn channel<Req, Res>(
    buffer: usize,
) -> (LocalTransportFactory<Req, Res>, LocalClientStream<Res, Req>) {
    let (tx, rx) = mpsc::channel(buffer);
    (
        LocalTransportFactory {
            rx: Receiver::Bounded(rx),
        },
        LocalClientStream {
            tx: Sender::Bounded(tx),
        },
    )
}

pub struct LocalClientStream<Req, Res> {
    tx: Sender<LocalTransport<Res, Req>>,
}

impl<Req: Debug + Send + 'static, Res: Debug + Send + 'static> LocalClientStream<Req, Res> {
    pub fn connect_unbounded(
        &self,
    ) -> Result<LocalTransport<Req, Res>, Box<dyn Error + Send + Sync>> {
        let (tx1, rx2) = mpsc::unbounded_channel();
        let (tx2, rx1) = mpsc::unbounded_channel();
        let transport = LocalTransport::<Res, Req> {
            tx: Sender::Unbounded(tx1),
            rx: Receiver::Unbounded(rx1),
        };
        match &self.tx {
            Sender::Bounded(tx) => tx.try_send(transport)?,
            Sender::Unbounded(tx) => tx.send(transport)?,
        }

        Ok(LocalTransport {
            tx: Sender::Unbounded(tx2),
            rx: Receiver::Unbounded(rx2),
        })
    }

    pub fn connect(
        &self,
        buffer: usize,
    ) -> Result<LocalTransport<Req, Res>, Box<dyn Error + Send + Sync>> {
        let (tx1, rx2) = mpsc::channel(buffer);
        let (tx2, rx1) = mpsc::channel(buffer);
        let transport = LocalTransport::<Res, Req> {
            tx: Sender::Bounded(tx1),
            rx: Receiver::Bounded(rx1),
        };
        match &self.tx {
            Sender::Bounded(tx) => tx.try_send(transport)?,
            Sender::Unbounded(tx) => tx.send(transport)?,
        }

        Ok(LocalTransport {
            tx: Sender::Bounded(tx2),
            rx: Receiver::Bounded(rx2),
        })
    }
}
