use futures::Stream;
use parity_tokio_ipc::Endpoint;
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

use super::get_socket_address;

pub struct Transport {
    endpoint: Endpoint,
}

impl Transport {
    pub fn new(app_id: impl AsRef<str>) -> Self {
        let endpoint = Endpoint::new(get_socket_address(app_id.as_ref(), ""));
        Self { endpoint }
    }

    pub fn incoming(
        self,
    ) -> io::Result<impl Stream<Item = io::Result<impl AsyncRead + AsyncWrite>> + 'static> {
        self.endpoint.incoming()
    }
}
