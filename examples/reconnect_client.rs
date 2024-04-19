use std::time::Duration;

use tower::reconnect::Reconnect;
use tower::util::BoxService;
use tower::{service_fn, BoxError, Service, ServiceExt};
use tower_rpc::transport::codec::{serde_codec, Codec};
use tower_rpc::transport::{tcp, Connect};
use tower_rpc::Client;

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let make_service = service_fn(move |_: ()| {
        Box::pin(async move {
            let socket = tcp::Connection::connect("127.0.0.1:8080".parse()?).await?;
            let client =
                Client::new(serde_codec::<usize, usize>(socket, Codec::Bincode)).create_pipeline();
            Ok::<_, BoxError>(client.boxed())
        })
    });

    let mut client = Reconnect::new::<BoxService<usize, usize, BoxError>, ()>(make_service, ());
    let mut i = 0;
    loop {
        i = match client.ready().await?.call(i).await {
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
