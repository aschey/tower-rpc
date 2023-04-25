use std::io;

use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::LengthDelimitedCodec;

mod codec;
pub use codec::*;
mod codec_wrapper;
pub use codec_wrapper::*;
mod ipc_client_stream;
mod ipc_request_handler;
pub use ipc_request_handler::*;
mod ipc_service;
mod server;
pub use server::*;
mod client;
pub use client::*;
mod tagged;
pub use tagged::*;

pub fn get_socket_address(id: &str, suffix: &str) -> String {
    let suffix_full = if suffix.is_empty() {
        "".to_owned()
    } else {
        format!("_{suffix}")
    };

    #[cfg(unix)]
    let addr = format!("/tmp/{id}{suffix_full}.sock");
    #[cfg(windows)]
    let addr = format!("\\\\.\\pipe\\{id}{suffix_full}");
    addr
}

pub fn codec_transport<Req, Res>(
    incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
) -> Transport<Req, Res, io::Error, io::Error>
where
    Req: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
    Res: Serialize + for<'de> Deserialize<'de> + Unpin + Send + 'static,
{
    let stream = tokio_util::codec::Framed::new(incoming, LengthDelimitedCodec::new());

    let stream = tokio_serde::Framed::new(stream, CodecWrapper::new(Codec::Bincode));
    Box::new(stream)
}

pub fn length_delimited_transport(
    incoming: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
) -> Transport<BytesMut, Bytes, io::Error, io::Error> {
    Box::new(tokio_util::codec::Framed::new(
        incoming,
        LengthDelimitedCodec::new(),
    ))
}

#[derive(Clone, Copy)]
pub enum ServerMode {
    Multiplex,
    Pipeline,
}
