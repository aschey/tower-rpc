use std::{future, io, marker::PhantomData, pin::Pin, task::Poll};

use async_trait::async_trait;
use background_service::{BackgroundServiceManager, ServiceContext};
use bytes::{Bytes, BytesMut};

use futures::Future;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::formats::Bincode;
use tokio_tower::{multiplex, pipeline};
use tokio_util::{codec::LengthDelimitedCodec, sync::CancellationToken};
use tower::{Service, ServiceBuilder, ServiceExt};
use tower_rpc::{
    handler_fn,
    transport::{
        ipc,
        local::{self, LocalTransport},
    },
    Client, Codec, CodecBuilder, CodecWrapper, RequestHandler, SerdeCodec, Server, ServerMode,
    StreamSink,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (server_transport, client_stream) = local::unbounded();
    let server = Server::new(
        server_transport,
        handler_fn(|req: String, cancellation_token: CancellationToken| async move { 0 }),
        ServerMode::Pipeline,
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    let mut client = Client::new(client_stream.connect()).create_pipeline();
    client.ready().await.unwrap();
    let a = client.call("test".to_owned()).await.unwrap();
    println!("{a:?}");
    cancellation_token.cancel();
    manager.join_on_cancel().await.unwrap();
}
