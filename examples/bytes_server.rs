use async_trait::async_trait;
use background_service::BackgroundServiceManager;
use bytes::{Bytes, BytesMut};
use futures::future;
use std::task::Poll;
use tokio_util::sync::CancellationToken;
use tower::{service_fn, BoxError};
use tower_rpc::{
    transport::{
        ipc::{self, OnConflict},
        CodecTransport,
    },
    LengthDelimitedCodec, MakeHandler, Request, Server,
};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = ipc::create_endpoint("test", OnConflict::Overwrite)?;

    let server = Server::pipeline(
        CodecTransport::new(transport, LengthDelimitedCodec),
        service_fn(Handler::make),
    );
    let mut context = manager.get_context();
    context.add_service(server);

    manager.cancel_on_signal().await?;
    Ok(())
}

#[derive(Default)]
struct Handler;

#[async_trait]
impl tower::Service<Request<BytesMut>> for Handler {
    type Response = Bytes;
    type Error = BoxError;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<BytesMut>) -> Self::Future {
        println!(
            "Request: {}",
            String::from_utf8(req.value.to_vec()).expect("Invalid string")
        );
        println!("Send: pong");
        future::ready(Ok(Bytes::from("pong")))
    }
}
