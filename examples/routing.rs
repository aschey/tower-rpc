use std::{convert::Infallible, future, time::Duration};

use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower::{service_fn, util::BoxService, ServiceExt};
use tower_rpc::{
    make_service_fn,
    transport::local::{self},
    CallRoute, Client, RouteMatch, RouteService, Server,
};

#[tokio::main]
pub async fn main() {
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
    context.add_service(server).await.unwrap();

    let mut client = Client::new(client_stream.connect()).create_pipeline();

    let mut i = 0;
    loop {
        client.ready().await.unwrap();
        i = client.call_route("/test1", i).await.unwrap();
        println!("Pong {i}");
        client.ready().await.unwrap();
        i = client.call_route("/test2", i).await.unwrap();
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
