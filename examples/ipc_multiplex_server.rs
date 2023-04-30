use std::{convert::Infallible, time::Duration};

use background_service::BackgroundServiceManager;

use rand::{rngs::SmallRng, Rng, SeedableRng};
use tokio_util::sync::CancellationToken;

use tower::service_fn;
use tower_rpc::{
    make_service_fn,
    transport::{ipc, CodecTransport},
    Codec, Request, SerdeCodec, Server, Tagged,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = ipc::Transport::new("test");

    let server = Server::multiplex(
        CodecTransport::new(
            transport.incoming().unwrap(),
            SerdeCodec::<Tagged<usize>, Tagged<usize>>::new(Codec::Bincode),
        ),
        make_service_fn(|| {
            service_fn(|req: Request<usize>| {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(
                        SmallRng::from_entropy().gen_range(1000..5000),
                    ))
                    .await;
                    println!("Ping {}", req.value);
                    Ok::<_, Infallible>(req.value)
                })
            })
        }),
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    manager.cancel_on_signal().await.unwrap();
}
