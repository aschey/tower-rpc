use std::convert::Infallible;
use std::time::Duration;

use background_service::BackgroundServiceManager;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError};
use tower_rpc::transport::ipc::{self, IpcSecurity, OnConflict, SecurityAttributes, ServerId};
use tower_rpc::transport::CodecTransport;
use tower_rpc::{make_service_fn, Codec, Request, SerdeCodec, Server, Tagged};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(
        cancellation_token.clone(),
        background_service::Settings::default(),
    );
    let transport = ipc::create_endpoint(
        ServerId("test"),
        SecurityAttributes::allow_everyone_create().expect("Failed to set security attributes"),
        OnConflict::Overwrite,
    )?;

    let server = Server::multiplex(
        CodecTransport::new(
            transport,
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
    context.add_service(server);

    manager.cancel_on_signal().await?;

    Ok(())
}
