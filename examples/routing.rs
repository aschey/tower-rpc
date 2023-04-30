use background_service::BackgroundServiceManager;
use std::{convert::Infallible, future, time::Duration};
use tokio_util::sync::CancellationToken;
use tower::{service_fn, util::BoxService, BoxError};
use tower_rpc::{
    make_service_fn,
    transport::local::{self},
    CallRoute, Client, RouteMatch, RouteService, Server,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded();

    let server = Server::pipeline(
        transport,
        make_service_fn(|| {
            let svc1 = BoxService::new(service_fn(|req: RouteMatch<usize>| {
                println!("Ping1 {}", req.value);
                future::ready(Ok::<_, Infallible>(req.value + 1))
            }));
            let svc2 = BoxService::new(service_fn(|req: RouteMatch<usize>| {
                println!("Ping2 {}", req.value);
                future::ready(Ok::<_, Infallible>(req.value + 1))
            }));

            RouteService::default()
                .with_route("/test1", svc1)
                .with_route("/test2", svc2)
        }),
    );

    let mut context = manager.get_context();
    context.add_service(server).await?;

    let mut client = Client::new(client_stream.connect()?).create_pipeline();

    let mut i = 0;
    loop {
        i = client.call_route_ready("/test1", i).await?;
        println!("Pong {i}");

        i = client.call_route_ready("/test2", i).await?;
        println!("Pong {i}");

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
