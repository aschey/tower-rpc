use std::time::Duration;
use tower::BoxError;
use tower_rpc::{
    serde_codec,
    transport::ipc::{self},
    Client, Codec, ReadyServiceExt,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport = ipc::connect("test").await?;
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
