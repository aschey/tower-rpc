use std::fmt::Debug;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::{future, io};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;
use futures::Future;
use tokio_util::sync::CancellationToken;
use tower::layer::layer_fn;
use tower::{BoxError, Service, ServiceBuilder};
use tower_rpc::transport::local::{self};
use tower_rpc::{make_service_fn, Client, ReadyServiceExt, Request, Server};
use tracing::metadata::LevelFilter;
use tracing::{info, span, Level};
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;
use tracing_tree::HierarchicalLayer;

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let layer = HierarchicalLayer::default()
        .with_writer(io::stdout)
        .with_indent_lines(true)
        .with_indent_amount(2)
        // .with_thread_names(true)
        .with_thread_ids(true)
        // .with_verbose_exit(true)
        // .with_verbose_entry(true)
        .with_targets(true);

    let subscriber = Registry::default().with(layer).with(LevelFilter::INFO);
    tracing::subscriber::set_global_default(subscriber)?;

    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(
        cancellation_token.clone(),
        background_service::Settings::default(),
    );
    let (transport, client_stream) = local::unbounded_channel();

    let server = Server::pipeline(
        transport,
        make_service_fn(|| {
            ServiceBuilder::default()
                .layer_fn(|inner| TracingService {
                    client: inner,
                    _phantom: PhantomData,
                })
                .service(Handler::default())
        }),
    );
    let mut context = manager.get_context();
    context.add_service(server);

    let client = Client::new(client_stream.connect_unbounded()?).create_pipeline();

    let mut client = ServiceBuilder::default()
        .layer(layer_fn(|client| TracingService {
            client,
            _phantom: Default::default(),
        }))
        .service(client);

    let mut i = 0;

    loop {
        i = client.call_ready(i).await?;
        info!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Default, Debug)]
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
        // let span = span!(Level::INFO, "handler", request = format!("{:?}", req.value));
        // let _span = span.enter();
        info!("Ping {}", req.value);
        future::ready(Ok(self.count.fetch_add(1, Ordering::SeqCst) + 1))
    }
}

#[derive(Debug)]
pub struct TracingService<S, Req>
where
    S: Service<Req>,
{
    client: S,
    _phantom: PhantomData<Req>,
}

impl<S, Req> Service<Req> for TracingService<S, Req>
where
    Req: Debug,
    S: Service<Req>,
    S::Response: Debug,
    S::Error: Debug,
    S::Future: Send + 'static,
{
    type Response = S::Response;

    type Error = S::Error;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let req_str = format!("{req:?}");
        let fut = self.client.call(req);
        Box::pin(async move {
            let span = span!(Level::INFO, "call", request = %req_str);
            let _span = span.enter();
            let start = Instant::now();
            let res = fut.await;
            info!("Request took {:?}", Instant::now().duration_since(start));
            info!("Response {response}", response = format!("{res:?}"));
            res
        })
    }
}
