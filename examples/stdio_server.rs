use std::{future, io, marker::PhantomData, pin::Pin, process::Stdio, task::Poll};

use async_trait::async_trait;
use background_service::{BackgroundServiceManager, ServiceContext};
use bytes::{Bytes, BytesMut};

use futures::{Future, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::formats::Bincode;
use tokio_tower::{multiplex, pipeline};
use tokio_util::{
    codec::{LengthDelimitedCodec, LinesCodec},
    sync::CancellationToken,
};
use tower::{Service, ServiceBuilder, ServiceExt};
use tower_rpc::{
    channel, codec_builder_fn, handler_fn,
    transport::{ipc, stdio::StdioTransport, CodecTransport},
    Client, Codec, CodecBuilder, CodecWrapper, RequestHandler, RequestHandlerStream, RequestStream,
    SerdeCodec, Server, ServerMode, StreamSink,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());

    let mut stream = RequestHandlerStream::default();
    let mut handler = stream.request_stream().unwrap();

    let server = Server::new(
        CodecTransport::new(
            StdioTransport::incoming(),
            codec_builder_fn(|s| {
                Box::new(tokio_util::codec::Framed::new(s, LinesCodec::default()))
            }),
        ),
        stream,
        ServerMode::Pipeline,
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();
    while let Some((req, res)) = handler.next().await {
        dbg!(req);
        res.respond("test".to_owned()).unwrap();
    }
}
