use std::time::Duration;

use background_service::BackgroundServiceManager;

use futures::StreamExt;
use tokio_util::sync::CancellationToken;

use tower::BoxError;
use tower_rpc::{
    transport::local::{self},
    Client, ReadyServiceExt, RequestHandlerStreamFactory, Server,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let (transport, client_stream) = local::unbounded();
    let mut handler = RequestHandlerStreamFactory::default();
    let mut request_stream = handler.request_stream().expect("Request stream missing");
    let server = Server::pipeline(transport, handler);

    let mut context = manager.get_context();
    context.add_service(server).await?;

    tokio::spawn(async move {
        let mut i = 0;
        while let Some((req, res)) = request_stream.next().await {
            println!("Ping {}", req.value);
            i += 1;
            res.respond(i).unwrap();
        }
    });

    let mut client = Client::new(client_stream.connect()?).create_pipeline();
    let mut i = 0;

    loop {
        i = client.call_ready(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
