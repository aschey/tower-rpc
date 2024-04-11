use std::time::Duration;

use tower::{BoxError, Service, ServiceExt};
use tower_rpc::transport::tcp;
use tower_rpc::{serde_codec, Client, Codec};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport = tcp::connect("127.0.0.1:8080").await?;
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
