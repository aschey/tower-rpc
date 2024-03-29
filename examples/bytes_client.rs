use std::time::Duration;

use bytes::Bytes;
use tower::BoxError;
use tower_rpc::transport::ipc::{self, ServerId};
use tower_rpc::{length_delimited_codec, Client, ReadyServiceExt};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport = ipc::connect(ServerId("test")).await?;
    let mut client = Client::new(length_delimited_codec(client_transport)).create_pipeline();

    loop {
        println!("Send: ping");
        let res = client.call_ready(Bytes::from("ping")).await?;
        println!("Response: {}", String::from_utf8(res.to_vec())?);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
