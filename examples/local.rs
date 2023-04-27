use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower::{Service, ServiceExt};
use tower_rpc::{
    transport::local::{self},
    Client, RequestHandler, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded();

    let server = Server::pipeline(transport, Handler::default());
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    let mut client = Client::new(client_stream.connect()).create_pipeline();
    let mut i = 0;

    loop {
        client.ready().await.unwrap();
        i = client.call(i).await.unwrap();
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Default)]
struct Handler {
    count: AtomicUsize,
}

#[async_trait]
impl RequestHandler for Handler {
    type Req = usize;

    type Res = usize;

    async fn handle_request(
        &self,
        request: Self::Req,
        _cancellation_token: CancellationToken,
    ) -> Self::Res {
        println!("Ping {request}");
        self.count.fetch_add(1, Ordering::SeqCst) + 1
    }
}
