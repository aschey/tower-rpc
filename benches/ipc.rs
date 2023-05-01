use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::{task::Poll, time::Instant};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use futures::future;
use tokio_util::sync::CancellationToken;

use tower::{service_fn, BoxError};
use tower_rpc::{
    serde_codec,
    transport::{
        ipc,
        tcp::{TcpStream, TcpTransport},
        CodecTransport,
    },
    Client, Codec, MakeHandler, ReadyServiceExt, Request, SerdeCodec, Server,
};

pub fn bench_calls(c: &mut Criterion) {
    let mut group = c.benchmark_group("calls");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token);
    let mut context = manager.get_context();
    rt.block_on(async {
        let transport = ipc::create_endpoint("test");

        let server = Server::pipeline(
            CodecTransport::new(
                transport.incoming().unwrap(),
                SerdeCodec::<usize, usize>::new(Codec::Bincode),
            ),
            service_fn(Handler::make),
        );
        context.add_service(server).await.unwrap();
    });

    group.bench_function("ipc", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let client_transport = ipc::connect("test").await.unwrap();
            let mut client = Client::new(serde_codec::<usize, usize>(
                client_transport,
                Codec::Bincode,
            ))
            .create_pipeline();
            let mut i = 0;
            let start = Instant::now();
            for _i in 0..iters {
                i = black_box(client.call_ready(i).await.unwrap());
            }
            start.elapsed()
        });
    });

    rt.block_on(async {
        manager.cancel().await.unwrap();
    });

    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token);
    let mut context = manager.get_context();

    rt.block_on(async move {
        let transport = TcpTransport::bind("127.0.0.1:8080".parse().unwrap())
            .await
            .unwrap();

        let server = Server::pipeline(
            CodecTransport::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
            service_fn(Handler::make),
        );
        context.add_service(server).await.unwrap();
    });

    group.bench_function("tcp", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let client_transport = TcpStream::connect("127.0.0.1:8080").await.unwrap();
            let mut client = Client::new(serde_codec::<usize, usize>(
                client_transport,
                Codec::Bincode,
            ))
            .create_pipeline();
            let mut i = 0;
            let start = Instant::now();
            for _i in 0..iters {
                i = black_box(client.call_ready(i).await.unwrap());
            }
            start.elapsed()
        });
    });

    rt.block_on(async {
        manager.cancel().await.unwrap();
    });
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
