use std::convert::Infallible;
use std::future;
use std::time::Duration;

use background_service::BackgroundServiceManager;
use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError, ServiceExt};
use tower_rpc::transport::ipc::{self, IpcSecurity, OnConflict, SecurityAttributes, ServerId};
use tower_rpc::transport::CodecTransport;
use tower_rpc::{
    keyed_codec, make_service_fn, CallKeyedRoute, Client, Codec, RouteMatch, RouteService, Server,
};

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
struct RequestMethod(#[serde(with = "http_serde::method")] Method);

impl From<Method> for RequestMethod {
    fn from(value: Method) -> Self {
        Self(value)
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct HttpResponse<T> {
    #[serde(with = "http_serde::status_code")]
    status: StatusCode,
    value: T,
}

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(
        cancellation_token.clone(),
        background_service::Settings::default(),
    );
    let transport = ipc::create_endpoint(
        ServerId("test"),
        SecurityAttributes::allow_everyone_create().expect("Failed to set security attributes"),
        OnConflict::Overwrite,
    )?;

    let codec = keyed_codec::<usize, HttpResponse<usize>, RequestMethod>(Codec::Bincode);

    let transport = CodecTransport::new(transport, codec.clone());

    let server = Server::pipeline(
        transport,
        make_service_fn(|| {
            let svc1 = service_fn(|req: RouteMatch<_, _>| {
                println!("Ping1 {}", req.value);
                future::ready(Ok::<_, Infallible>(HttpResponse {
                    status: StatusCode::OK,
                    value: req.value + 1,
                }))
            })
            .boxed();
            let svc2 = service_fn(|req: RouteMatch<_, _>| {
                println!("Ping2 {}", req.value);
                future::ready(Ok::<_, Infallible>(HttpResponse {
                    status: StatusCode::OK,
                    value: req.value + 1,
                }))
            })
            .boxed();

            RouteService::with_keys()
                .with_route(Method::GET, "/test1", svc1)
                .with_route(Method::POST, "/test2", svc2)
        }),
    );

    let mut context = manager.get_context();
    context.add_service(server);

    let client_transport = ipc::connect(ServerId("test")).await?;
    let mut client = Client::new(codec.client(client_transport)).create_pipeline();

    let mut i = 0;
    loop {
        i = client
            .call_route_ready(Method::GET, "/test1", i)
            .await?
            .value;
        println!("Pong {i}");

        i = client
            .call_route_ready(Method::POST, "/test2", i)
            .await?
            .value;
        println!("Pong {i}");

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
