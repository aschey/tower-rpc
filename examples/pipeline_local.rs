use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};
use tower_rpc::{
    handler_fn,
    transport::local::{self},
    Client, Server, ServerMode,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (server_transport, client_stream) = local::unbounded();
    let server = Server::new(
        server_transport,
        handler_fn(|_req: String, _cancellation_token: CancellationToken| async move { 0 }),
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
