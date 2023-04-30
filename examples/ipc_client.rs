use std::time::Duration;

use parity_tokio_ipc::Endpoint;

use tower_rpc::{serde_codec, transport::ipc::get_socket_address, Client, Codec, ReadyServiceExt};

#[tokio::main]
pub async fn main() {
    let addr = get_socket_address("test", "");
    let client_transport = Endpoint::connect(addr.clone()).await.unwrap();
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
