use std::convert::Infallible;
use std::future;
use std::time::Duration;

use background_service::BackgroundServiceManager;
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError};
use tower_rpc::transport::local::{self};
use tower_rpc::{make_service_fn, Client, ReadyServiceExt, Request, Server};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(
        cancellation_token.clone(),
        background_service::Settings::default(),
    );
    let (transport, client_stream) = local::unbounded_channel();

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
    context.add_service(server);

    let mut client = Client::new(client_stream.connect_unbounded()?).create_pipeline();
    let mut i = 0;

    loop {
        i = client.call_ready(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
