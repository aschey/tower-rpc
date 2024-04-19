use std::time::Duration;

use bytes::Bytes;
use tower::{BoxError, Service, ServiceExt};
use tower_rpc::transport::codec::length_delimited_codec;
use tower_rpc::transport::ipc::{self, ServerId};
use tower_rpc::transport::Connect;
use tower_rpc::Client;
#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport =
        ipc::Connection::connect(ipc::ConnectionParams::new(ServerId("test"))?).await?;
    let mut client = Client::new(length_delimited_codec(client_transport)).create_pipeline();

    loop {
        println!("Send: ping");
        let res = client.ready().await?.call(Bytes::from("ping")).await?;
        println!("Response: {}", String::from_utf8(res.to_vec()).unwrap());
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
