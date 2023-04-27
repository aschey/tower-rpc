use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};
use tower_rpc::{
    handler_fn, serde_codec,
    transport::{ipc, CodecTransport},
    Client, Codec, SerdeCodec, Server, ServerMode, Tagged,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = ipc::Transport::new("test");
    let server = Server::new(
        CodecTransport::new(
            transport.incoming().unwrap(),
            SerdeCodec::<Tagged<String>, Tagged<i32>>::new(Codec::Bincode),
        ),
        handler_fn(
            |_req: Tagged<String>, _cancellation_token: CancellationToken| async move {
                Tagged::from(0)
            },
        ),
        ServerMode::Multiplex,
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();
    let client_transport = ipc::ClientStream::new("test");

    let mut client = Client::new(serde_codec::<Tagged<i32>, Tagged<String>>(
        client_transport,
        Codec::Bincode,
    ))
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
