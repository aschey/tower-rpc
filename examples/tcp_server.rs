use std::future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError};
use tower_rpc::transport::{tcp, CodecTransport};
use tower_rpc::{Codec, MakeHandler, Request, SerdeCodec, Server};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(
        cancellation_token.clone(),
        background_service::Settings::default(),
    );
    let transport = tcp::create_endpoint("127.0.0.1:8080".parse()?).await?;

    let server = Server::pipeline(
        CodecTransport::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
        service_fn(Handler::make),
    );
    let mut context = manager.get_context();
    context.add_service(server);

    manager.cancel_on_signal().await?;
    Ok(())
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

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<usize>) -> Self::Future {
        println!("Ping {}", req.value);

        future::ready(Ok(self.count.fetch_add(1, Ordering::SeqCst) + 1))
    }
}
