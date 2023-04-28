use std::time::Duration;

use rand::{rngs::SmallRng, Rng, SeedableRng};
use tower::{buffer::Buffer, Service, ServiceExt};
use tower_rpc::{serde_codec, transport::ipc, Client, Codec, Tagged};

#[tokio::main]
pub async fn main() {
    let client_transport = ipc::ClientStream::new("test");
    let client = Client::new(serde_codec::<Tagged<usize>, Tagged<usize>>(
        client_transport,
        Codec::Bincode,
    ))
    .create_multiplex();
    let client = Buffer::new(client, 10);
    let mut i = 0;

    loop {
        let mut client_ = client.clone();
        tokio::task::spawn(async move {
            client_.ready().await.unwrap();
            let res = client_.call(i).await.unwrap();
            println!("Ping {i} Pong {res}");
        });

        i += 1;
        tokio::time::sleep(Duration::from_millis(
            SmallRng::from_entropy().gen_range(0..100),
        ))
        .await;
    }
}
