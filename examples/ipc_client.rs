use std::time::Duration;

use tower::{BoxError, Service, ServiceExt};
use tower_rpc::transport::codec::{Codec, SerdeCodec};
use tower_rpc::transport::ipc::{self, ServerId};
use tower_rpc::transport::Connect;
use tower_rpc::Client;

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport =
        ipc::Connection::connect(ipc::ConnectionParams::new(ServerId("test"))?).await?;

    let mut client =
        Client::new(SerdeCodec::<usize, usize>::new(Codec::Bincode).client(client_transport))
            .create_pipeline();
    let mut i = 0;
    loop {
        i = client.ready().await?.call(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
