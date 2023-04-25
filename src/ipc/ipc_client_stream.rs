use parity_tokio_ipc::{Connection, Endpoint};
use std::{
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Poll, Waker},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite};

use super::get_socket_address;

pub struct IpcClientStream {
    waker: Arc<Mutex<Option<Waker>>>,
    connection: Arc<Mutex<Option<Connection>>>,
    addr: String,
}

impl IpcClientStream {
    pub fn new(app_id: impl AsRef<str>) -> Self {
        let addr = get_socket_address(app_id.as_ref(), "");
        let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
        let waker_ = waker.clone();
        let connection: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
        let connection_ = connection.clone();
        try_connect(addr.clone(), waker_, connection_);
        Self {
            addr,
            waker,
            connection,
        }
    }

    fn handle_connection_poll<T>(
        &self,
        cx: &mut std::task::Context<'_>,
        poll_result: std::task::Poll<std::io::Result<T>>,
        mut connection: MutexGuard<Option<Connection>>,
    ) -> std::task::Poll<std::io::Result<T>> {
        match poll_result {
            res @ Poll::Ready(Ok(_)) => res,
            Poll::Ready(Err(_)) => {
                *connection = None;
                try_connect(
                    self.addr.clone(),
                    self.waker.clone(),
                    self.connection.clone(),
                );
                let mut waker = self.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
            res @ Poll::Pending => res,
        }
    }
}

impl AsyncRead for IpcClientStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let mut connection = self.connection.lock().unwrap();
        match &mut *connection {
            Some(conn) => {
                let poll_result = Pin::new(conn).poll_read(cx, buf);
                self.handle_connection_poll(cx, poll_result, connection)
            }
            None => {
                let mut waker = self.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl AsyncWrite for IpcClientStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let mut connection = self.connection.lock().unwrap();
        match &mut *connection {
            Some(conn) => {
                let poll_result = Pin::new(conn).poll_write(cx, buf);
                self.handle_connection_poll(cx, poll_result, connection)
            }
            None => {
                let mut waker = self.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let mut connection = self.connection.lock().unwrap();
        match &mut *connection {
            Some(conn) => {
                let poll_result = Pin::new(conn).poll_flush(cx);
                self.handle_connection_poll(cx, poll_result, connection)
            }
            None => {
                let mut waker = self.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let mut connection = self.connection.lock().unwrap();
        match &mut *connection {
            Some(conn) => {
                let poll_result = Pin::new(conn).poll_shutdown(cx);
                self.handle_connection_poll(cx, poll_result, connection)
            }
            None => {
                let mut waker = self.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

fn try_connect(
    addr: String,
    waker_: Arc<Mutex<Option<Waker>>>,
    connection_: Arc<Mutex<Option<Connection>>>,
) {
    tokio::spawn(async move {
        loop {
            if let Ok(connection) = Endpoint::connect(&addr).await {
                let waker = waker_.lock().unwrap().take();
                let mut c = connection_.lock().unwrap();
                *c = Some(connection);
                if let Some(w) = waker {
                    w.wake();
                    return;
                } else {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
}
