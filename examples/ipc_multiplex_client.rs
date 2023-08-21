use std::time::Duration;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use tower::buffer::Buffer;
use tower::BoxError;
use tower_rpc::transport::ipc::{self, ConnectionId};
use tower_rpc::{Client, Codec, ReadyServiceExt, SerdeCodec, Tagged};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let client_transport = ipc::connect(ConnectionId("test".to_owned())).await?;
    let client = Client::new(
        SerdeCodec::<Tagged<usize>, Tagged<usize>>::new(Codec::Bincode).client(client_transport),
    )
    .create_multiplex();
    let client = Buffer::new(client, 10);
    let mut i = 0;

    loop {
        let mut client_ = client.clone();
        tokio::task::spawn(async move {
            let res = client_.call_ready(i).await.unwrap();
            println!("Ping {i} Pong {res}");
        });

        i += 1;
        tokio::time::sleep(Duration::from_millis(
            SmallRng::from_entropy().gen_range(0..100),
        ))
        .await;
    }
}
