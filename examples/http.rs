use std::future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use background_service::BackgroundServiceManager;
use http::Method;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tower_rpc::http::HttpAdapter;
use tower_rpc::transport::codec::{Codec, CodecSerializer};
use tower_rpc::transport::{tcp, Bind};
use tower_rpc::{make_service_fn, Keyed, RouteMatch, RouteService};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    tracing_subscriber::fmt().init();
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(
        cancellation_token.clone(),
        background_service::Settings::default(),
    );
    let context = manager.get_context();
    let transport = tcp::Endpoint::bind("127.0.0.1:8080".parse()?).await?;

    let handler = Handler {
        count: Arc::new(AtomicUsize::new(0)),
    };
    let server = tower_rpc::http::Server::new(
        transport,
        make_service_fn(move || {
            ServiceBuilder::default()
                .layer(TraceLayer::new_for_http())
                .layer_fn(|inner| {
                    HttpAdapter::new(inner, CodecSerializer::new(Codec::Json), context.clone())
                })
                .service(RouteService::with_keys().with_route(
                    Method::POST,
                    "/test1",
                    handler.clone(),
                ))
        }),
    );

    let mut context = manager.get_context();
    context.add_service(server);

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

impl tower::Service<RouteMatch<Message, Keyed<Method>>> for Handler {
    type Response = Message;
    type Error = BoxError;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: RouteMatch<Message, Keyed<Method>>) -> Self::Future {
        println!("Ping {:?}", req.value);

        future::ready(Ok(Message {
            count: (self.count.fetch_add(1, Ordering::SeqCst) + 1),
        }))
    }
}
