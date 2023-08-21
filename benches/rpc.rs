use std::task::Poll;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures::future;
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError};
use tower_rpc::transport::ipc::{self, IpcSecurity, OnConflict, SecurityAttributes, ServerId};
use tower_rpc::transport::{tcp, CodecTransport};
use tower_rpc::{
    serde_codec, Client, Codec, MakeHandler, ReadyServiceExt, Request, SerdeCodec, Server,
};

pub fn bench_calls(c: &mut Criterion) {
    bench_inner(c).expect("Failed to run")
}

fn bench_inner(c: &mut Criterion) -> Result<(), BoxError> {
    let mut group = c.benchmark_group("calls");
    group
        .measurement_time(Duration::from_secs(20))
        .sample_size(1000);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()?;

    let cancellation_token = CancellationToken::default();
    let manager =
        BackgroundServiceManager::new(cancellation_token, background_service::Settings::default());
    let mut context = manager.get_context();
    rt.block_on(async {
        let transport = ipc::create_endpoint(
            ServerId("test"),
            SecurityAttributes::allow_everyone_create().expect("Failed to set security attributes"),
            OnConflict::Overwrite,
        )?;

        let server = Server::pipeline(
            CodecTransport::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
            service_fn(Handler::make),
        );
        context.add_service(server);
        Ok::<_, BoxError>(())
    })?;

    group.bench_function("ipc", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let client_transport = ipc::connect(ServerId("test"))
                .await
                .expect("Failed to connect");
            let mut client = Client::new(serde_codec::<usize, usize>(
                client_transport,
                Codec::Bincode,
            ))
            .create_pipeline();
            let mut i = 0;
            tokio::spawn(async move {
                let start = Instant::now();
                for _i in 0..iters {
                    i = black_box(client.call_ready(i).await.expect("Failed to call service"));
                }
                start.elapsed()
            })
            .await
            .expect("Failed to spawn task")
        });
    });

    rt.block_on(async { manager.cancel().await })?;

    let cancellation_token = CancellationToken::default();
    let manager =
        BackgroundServiceManager::new(cancellation_token, background_service::Settings::default());
    let mut context = manager.get_context();

    rt.block_on(async move {
        let transport = tcp::create_endpoint("127.0.0.1:8080".parse()?).await?;

        let server = Server::pipeline(
            CodecTransport::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
            service_fn(Handler::make),
        );
        context.add_service(server);

        Ok::<_, BoxError>(())
    })?;

    group.bench_function("tcp", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let client_transport = tcp::connect("127.0.0.1:8080")
                .await
                .expect("Failed to connect");
            let mut client = Client::new(serde_codec::<usize, usize>(
                client_transport,
                Codec::Bincode,
            ))
            .create_pipeline();
            let mut i = 0;
            tokio::spawn(async move {
                let start = Instant::now();
                for _i in 0..iters {
                    i = black_box(client.call_ready(i).await.expect("Failed to call service"));
                }
                start.elapsed()
            })
            .await
            .expect("Failed spawn")
        });
    });
    group.finish();

    rt.block_on(async { manager.cancel().await })?;

    Ok(())
}

#[derive(Default)]
struct Handler;

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
        future::ready(Ok(req.value))
    }
}

criterion_group!(benches, bench_calls);
criterion_main!(benches);
