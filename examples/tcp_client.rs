use std::time::Duration;

use tower::{BoxError, Service, ServiceExt};
use tower_rpc::transport::tcp;
use tower_rpc::Client;
use transport_async::codec::{serde_codec, Codec};
use transport_async::Connect;

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport = tcp::Connection::connect("127.0.0.1:8080".parse()?).await?;
    let mut client = Client::new(serde_codec::<usize, usize>(
        client_transport,
        Codec::Bincode,
    ))
    .create_pipeline();
    let mut i = 0;

    loop {
        i = client.ready().await?.call(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
