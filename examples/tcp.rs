use background_service::BackgroundServiceManager;

use tokio::net::TcpStream;

use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};
use tower_rpc::{
    channel, serde_codec,
    transport::{tcp::TcpTransport, CodecTransport},
    Client, Codec, SerdeCodec, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = TcpTransport::bind("127.0.0.1:8080".parse().unwrap()).await;

    let (tx, mut rx) = channel();
    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            dbg!(req);
        }
    });
    let server = Server::pipeline(
        CodecTransport::new(transport, SerdeCodec::<String, ()>::new(Codec::Bincode)),
        tx,
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    let mut client = Client::new(serde_codec::<(), String>(
        TcpStream::connect("127.0.0.1:8080").await.unwrap(),
        Codec::Bincode,
    ))
    .create_pipeline();
    client.ready().await.unwrap();
    let a = client.call("test".to_owned()).await.unwrap();
    println!("{a:?}");
    cancellation_token.cancel();
    manager.join_on_cancel().await.unwrap();
}
