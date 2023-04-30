use std::time::Duration;

use parity_tokio_ipc::Endpoint;

use tower::BoxError;
use tower_rpc::{serde_codec, transport::ipc::get_socket_address, Client, Codec, ReadyServiceExt};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let addr = get_socket_address("test", "");
    let client_transport = Endpoint::connect(addr.clone()).await?;
    let mut client = Client::new(serde_codec::<usize, usize>(
        client_transport,
        Codec::Bincode,
    ))
    .create_pipeline();
    let mut i = 0;
    loop {
        i = client.call_ready(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
