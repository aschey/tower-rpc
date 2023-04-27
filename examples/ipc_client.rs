use std::time::Duration;

use tower::{Service, ServiceExt};
use tower_rpc::{serde_codec, transport::ipc, Client, Codec};

#[tokio::main]
pub async fn main() {
    let client_transport = ipc::ClientStream::new("test");
    let mut client = Client::new(serde_codec::<usize, usize>(
        client_transport,
        Codec::Bincode,
    ))
    .create_pipeline();
    let mut i = 0;

    loop {
        client.ready().await.unwrap();
        i = client.call(i).await.unwrap();
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
