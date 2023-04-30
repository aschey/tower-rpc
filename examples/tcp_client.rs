use std::time::Duration;

use tokio::net::TcpStream;

use tower_rpc::{serde_codec, Client, Codec, ReadyServiceExt};

#[tokio::main]
pub async fn main() {
    let client_transport = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut client = Client::new(serde_codec::<usize, usize>(
        client_transport,
        Codec::Bincode,
    ))
    .create_pipeline();
    let mut i = 0;

    loop {
        i = client.call_ready(i).await.unwrap();
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
