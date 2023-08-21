use std::time::Duration;

use tower::BoxError;
use tower_rpc::transport::ipc::{self};
use tower_rpc::{Client, Codec, ReadyServiceExt, SerdeCodec};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport = ipc::connect("test").await?;

    let mut client =
        Client::new(SerdeCodec::<usize, usize>::new(Codec::Bincode).client(client_transport))
            .create_pipeline();
    let mut i = 0;
    loop {
        i = client.call_ready(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
