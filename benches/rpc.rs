use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use background_service::BackgroundServiceManager;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures::future;
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError, Service, ServiceExt};
use tower_rpc::transport::codec::{serde_codec, Codec, CodecStream, SerdeCodec};
use tower_rpc::transport::ipc::{self, IpcSecurity, OnConflict, SecurityAttributes, ServerId};
use tower_rpc::transport::{tcp, Bind, Connect};
use tower_rpc::{Client, MakeHandler, Request, Server};

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
    let _guard = rt.enter();

    let cancellation_token = CancellationToken::default();
    let manager =
        BackgroundServiceManager::new(cancellation_token, background_service::Settings::default());
    let mut context = manager.get_context();
    rt.block_on(async {
        let transport = ipc::Endpoint::bind(ipc::EndpointParams::new(
            ServerId("test"),
            SecurityAttributes::allow_everyone_create()?,
            OnConflict::Overwrite,
        )?)
        .await?;

        let server = Server::pipeline(
            CodecStream::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
            service_fn(Handler::make),
        );
        context.add_service(server);
        Ok::<_, BoxError>(())
    })?;

    group.bench_function("ipc", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let client_transport =
                ipc::Connection::connect(ipc::ConnectionParams::new(ServerId("test")).unwrap())
                    .await
                    .unwrap();
            let mut client = Client::new(serde_codec::<usize, usize>(
                client_transport,
                Codec::Bincode,
            ))
            .create_pipeline();
            let mut i = 0;
            tokio::spawn(async move {
                let start = Instant::now();
                for _i in 0..iters {
                    i = black_box(
                        client
                            .ready()
                            .await
                            .unwrap()
                            .call(i)
                            .await
                            .expect("Failed to call service"),
                    );
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
        let transport = tcp::Endpoint::bind("127.0.0.1:8080".parse()?).await?;

        let server = Server::pipeline(
            CodecStream::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
            service_fn(Handler::make),
        );
        context.add_service(server);

        Ok::<_, BoxError>(())
    })?;

    group.bench_function("tcp", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let client_transport = tcp::Connection::connect("127.0.0.1:8080".parse().unwrap())
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
                    i = black_box(
                        client
                            .ready()
                            .await
                            .unwrap()
                            .call(i)
                            .await
                            .expect("Failed to call service"),
                    );
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

impl tower::Service<Request<usize>> for Handler {
    type Response = usize;
    type Error = BoxError;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<usize>) -> Self::Future {
        future::ready(Ok(req.value))
    }
}

criterion_group!(benches, bench_calls);
criterion_main!(benches);
