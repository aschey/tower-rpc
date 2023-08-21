use std::time::Duration;

use background_service::BackgroundServiceManager;
use tokio_util::sync::CancellationToken;
use tower::BoxError;
use tower_rpc::transport::local::{self};
use tower_rpc::{channel, Client, ReadyServiceExt, Server};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded_channel();
    let (tx, mut rx) = channel::<usize>();

    let server = Server::pipeline(transport, tx);

    let mut context = manager.get_context();
    context.add_service(server);

    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            println!("Ping {}", req.value);
        }
    });

    let mut client = Client::new(client_stream.connect_unbounded()?).create_pipeline();
    let mut i = 0;

    loop {
        client.call_ready(i).await?;
        i += 1;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
