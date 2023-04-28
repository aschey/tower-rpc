use std::{
    convert::Infallible,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
    time::Duration,
};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use futures::future;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use tokio_util::sync::CancellationToken;

use tower::{service_fn, BoxError};
use tower_rpc::{
    make_service_fn,
    transport::{ipc, CodecTransport},
    Codec, Request, SerdeCodec, Server, Tagged,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = ipc::Transport::new("test");

    let server = Server::multiplex(
        CodecTransport::new(
            transport.incoming().unwrap(),
            SerdeCodec::<Tagged<usize>, Tagged<usize>>::new(Codec::Bincode),
        ),
        make_service_fn(|| {
            service_fn(|req: Request<usize>| {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(
                        SmallRng::from_entropy().gen_range(1000..5000),
                    ))
                    .await;
                    Ok::<_, Infallible>(req.value)
                })
            })
        }),
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

    fn call(&mut self, _req: Request<usize>) -> Self::Future {
        future::ready(Ok(self.count.fetch_add(1, Ordering::SeqCst) + 1))
    }
}
