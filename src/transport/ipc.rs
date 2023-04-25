use std::io;

use futures::Stream;
use parity_tokio_ipc::Endpoint;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::ipc::get_socket_address;

pub struct IpcTransport {
    endpoint: Endpoint,
}

impl IpcTransport {
    pub fn new(app_id: impl AsRef<str>) -> Self {
        let endpoint = Endpoint::new(get_socket_address(app_id.as_ref(), ""));
        Self { endpoint }
    }

    pub fn incoming(
        self,
    ) -> io::Result<impl Stream<Item = std::io::Result<impl AsyncRead + AsyncWrite>> + 'static>
    {
        self.endpoint.incoming()
    }
}
