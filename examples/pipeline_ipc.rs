use std::{future, io, marker::PhantomData, pin::Pin, task::Poll};

use async_trait::async_trait;
use background_service::{BackgroundServiceManager, ServiceContext};
use bytes::{Bytes, BytesMut};

use futures::{Future, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::formats::Bincode;
use tokio_tower::{multiplex, pipeline};
use tokio_util::{codec::LengthDelimitedCodec, sync::CancellationToken};
use tower::{Service, ServiceBuilder, ServiceExt};
use tower_rpc::{
    handler_fn,
    transport::{ipc, CodecTransport},
    Client, Codec, CodecBuilder, CodecWrapper, RequestHandler, RequestHandlerStream, RequestStream,
    SerdeCodec, Server, ServerMode, StreamSink,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = ipc::Transport::new("test");
    let mut handler = RequestHandlerStream::default();
    let mut stream = handler.request_stream().unwrap();
    tokio::spawn(async move {
        while let Some((req, res)) = stream.next().await {
            dbg!(req);
            res.respond(0).unwrap();
        }
    });
    let server = Server::new(
        CodecTransport::new(
            transport.incoming().unwrap(),
            SerdeCodec::<String, i32>::new(Codec::Bincode),
        ),
        handler,
        ServerMode::Pipeline,
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();
    let client_transport = ipc::ClientStream::new("test");
    let mut client =
        Client::new(SerdeCodec::<i32, String>::new(Codec::Bincode).build_codec(client_transport))
            .create_pipeline();
    client.ready().await.unwrap();
    let a = client.call("test".to_owned()).await.unwrap();
    println!("{a:?}");
    cancellation_token.cancel();
    manager.join_on_cancel().await.unwrap();
}
