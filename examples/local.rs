use std::future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use background_service::BackgroundServiceManager;
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError, Service, ServiceExt};
use tower_rpc::transport::local;
use tower_rpc::{Client, MakeHandler, Request, Server};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(
        cancellation_token.clone(),
        background_service::Settings::default(),
    );
    let (transport, client_stream) = local::unbounded_channel();

    let server = Server::pipeline(transport, service_fn(Handler::make));
    let mut context = manager.get_context();
    context.add_service(server);

    let mut client = Client::new(client_stream.connect_unbounded()?).create_pipeline();
    let mut i = 0;

    loop {
        i = client.ready().await?.call(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Default)]
struct Handler {
    count: AtomicUsize,
}

impl tower::Service<Request<usize>> for Handler {
    type Response = usize;
    type Error = BoxError;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<usize>) -> Self::Future {
        println!("Ping {}", req.value);

        future::ready(Ok(self.count.fetch_add(1, Ordering::SeqCst) + 1))
    }
}
