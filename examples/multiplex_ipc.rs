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
    handler_fn, transport::ipc, Client, Codec, CodecBuilder, CodecWrapper, RequestHandler,
    SerdeCodec, Server, ServerMode, StreamSink, Tagged,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = ipc::Transport::new("test");
    let server = Server::new(
        transport.incoming().unwrap(),
        handler_fn(
            |req: Tagged<String>, cancellation_token: CancellationToken| async move { Tagged::from(0) },
        ),
        SerdeCodec::<Tagged<String>, Tagged<i32>>::new(Codec::Bincode),
        ServerMode::Multiplex,
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();
    let client_transport = ipc::ClientStream::new("test");

    let mut client = Client::new(
        client_transport,
        SerdeCodec::<Tagged<i32>, Tagged<String>>::new(Codec::Bincode),
    )
    .create_multiplex();

    let a = client
        .ready()
        .await
        .unwrap()
        .call("test".to_owned().into())
        .await
        .unwrap();
    println!("{a:?}");
    cancellation_token.cancel();
    manager.join_on_cancel().await.unwrap();
}
