use background_service::BackgroundServiceManager;
use std::{convert::Infallible, future, time::Duration};
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError, ServiceExt};
use tower_rpc::{
    make_service_fn,
    transport::local::{self},
    CallRoute, Client, RouteMatch, RouteService, Server,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded_channel();

    let server = Server::pipeline(
        transport,
        make_service_fn(|| {
            let svc1 = service_fn(|req: RouteMatch<_>| {
                println!("Ping1 {}", req.value);
                future::ready(Ok::<_, Infallible>(req.value + 1))
            })
            .boxed();
            let svc2 = service_fn(|req: RouteMatch<_>| {
                println!("Ping2 {}", req.value);
                future::ready(Ok::<_, Infallible>(req.value + 1))
            })
            .boxed();

            RouteService::default()
                .with_route("/test1", svc1)
                .unwrap()
                .with_route("/test2", svc2)
                .unwrap()
        }),
    );

    let mut context = manager.get_context();
    context.add_service(server);

    let mut client = Client::new(client_stream.connect_unbounded()?).create_pipeline();

    let mut i = 0;
    loop {
        i = client.call_route_ready("/test1", i).await?;
        println!("Pong {i}");

        i = client.call_route_ready("/test2", i).await?;
        println!("Pong {i}");

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
