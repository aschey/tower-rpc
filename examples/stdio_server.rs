use std::{
    future,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower::{service_fn, BoxError};
use tower_rpc::{
    transport::{stdio::StdioTransport, CodecTransport},
    Codec, MakeHandler, Request, SerdeCodec, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = StdioTransport::incoming();

    let server = Server::pipeline(
        CodecTransport::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
        service_fn(Handler::make),
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    manager.cancel_on_signal().await.unwrap();
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
