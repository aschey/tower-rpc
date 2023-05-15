use std::time::Duration;

use parity_tokio_ipc::Endpoint;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use tower::{buffer::Buffer, BoxError};
use tower_rpc::{
    transport::ipc::get_socket_address, Client, Codec, ReadyServiceExt, SerdeCodec, Tagged,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let addr = get_socket_address("test", "");
    let client_transport = Endpoint::connect(addr.clone()).await?;
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
