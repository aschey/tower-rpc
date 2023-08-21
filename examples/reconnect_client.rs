use std::time::Duration;

use tower::reconnect::Reconnect;
use tower::util::BoxService;
use tower::{service_fn, BoxError, ServiceExt};
use tower_rpc::transport::tcp;
use tower_rpc::{serde_codec, Client, Codec, ReadyServiceExt};

#[tokio::main]
pub async fn main() {
    let make_service = service_fn(move |_: ()| {
        Box::pin(async move {
            let socket = tcp::connect("127.0.0.1:8080").await?;
            let client =
                Client::new(serde_codec::<usize, usize>(socket, Codec::Bincode)).create_pipeline();
            Ok::<_, BoxError>(client.boxed())
        })
    });

    let mut client = Reconnect::new::<BoxService<usize, usize, BoxError>, ()>(make_service, ());
    let mut i = 0;
    loop {
        i = match client.call_ready(i).await {
            Ok(i) => i,
            Err(e) => {
                println!("Err {e:?}");
                i
            }
        };
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
