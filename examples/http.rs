use std::{
    future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::Poll,
};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use http::Method;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tower_rpc::{
    http::HttpAdapter, make_service_fn, transport::tcp::TcpTransport, Codec, CodecSerializer,
    Keyed, RouteMatch, RouteService,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let context = manager.get_context();
    let transport = TcpTransport::bind("127.0.0.1:8080".parse()?).await?;

    let server = tower_rpc::http::Server::new(
        transport,
        make_service_fn(move || {
            ServiceBuilder::default()
                .layer(TraceLayer::new_for_http())
                .layer_fn(|inner| {
                    HttpAdapter::new(inner, CodecSerializer::new(Codec::Json), context.clone())
                })
                .service(
                    RouteService::with_keys()
                        .with_route(
                            Method::POST,
                            "/test1",
                            Handler {
                                count: Arc::new(AtomicUsize::new(0)),
                            },
                        )
                        .unwrap(),
                )
        }),
    );

    let mut context = manager.get_context();
    context.add_service(server).await?;

    manager.cancel_on_signal().await?;

    Ok(())
}

#[derive(Default, Clone)]
struct Handler {
    count: Arc<AtomicUsize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    count: usize,
}

#[async_trait]
impl tower::Service<RouteMatch<Message, Keyed<Method>>> for Handler {
    type Response = Message;
    type Error = BoxError;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: RouteMatch<Message, Keyed<Method>>) -> Self::Future {
        println!("Ping {:?}", req.value);

        future::ready(Ok(Message {
            count: (self.count.fetch_add(1, Ordering::SeqCst) + 1),
        }))
    }
}
