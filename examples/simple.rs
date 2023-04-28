use std::{convert::Infallible, future, time::Duration};

use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower::{service_fn, Service, ServiceExt};
use tower_rpc::{
    make_service_fn,
    transport::local::{self},
    Client, Request, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded();

    let server = Server::pipeline(
        transport,
        make_service_fn(|| {
            service_fn(|req: Request<usize>| {
                println!("Ping {}", req.value);
                future::ready(Ok::<_, Infallible>(req.value + 1))
            })
        }),
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    let mut client = Client::new(client_stream.connect()).create_pipeline();
    let mut i = 0;

    loop {
        client.ready().await.unwrap();
        i = client.call(i).await.unwrap();
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
