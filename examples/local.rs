use std::{
    future,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
    time::Duration,
};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower::{service_fn, BoxError};
use tower_rpc::{
    transport::local::{self},
    Client, MakeHandler, ReadyServiceExt, Request, Server,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded_channel();

    let server = Server::pipeline(transport, service_fn(Handler::make));
    let mut context = manager.get_context();
    context.add_service(server);

    let mut client = Client::new(client_stream.connect_unbounded()?).create_pipeline();
    let mut i = 0;

    loop {
        i = client.call_ready(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Default)]
struct Handler {
    count: AtomicUsize,
}

#[async_trait]
impl tower::Service<Request<usize>> for Handler {
    type Response = usize;
    type Error = BoxError;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<usize>) -> Self::Future {
        println!("Ping {}", req.value);

        future::ready(Ok(self.count.fetch_add(1, Ordering::SeqCst) + 1))
    }
}
