use std::time::Duration;

use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower::{Service, ServiceExt};
use tower_rpc::{
    channel,
    transport::local::{self},
    Client, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded();
    let (tx, mut rx) = channel::<usize>();

    let server = Server::pipeline(transport, tx);

    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            println!("Ping {}", req.value);
        }
    });

    let mut client = Client::new(client_stream.connect()).create_pipeline();
    let mut i = 0;

    loop {
        client.ready().await.unwrap();
        client.call(i).await.unwrap();
        i += 1;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
