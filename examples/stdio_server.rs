use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower_rpc::{
    transport::{stdio::StdioTransport, CodecTransport},
    Codec, RequestHandler, SerdeCodec, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = StdioTransport::incoming();

    let server = Server::pipeline(
        CodecTransport::new(transport, SerdeCodec::<usize, usize>::new(Codec::Bincode)),
        Handler::default(),
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();

    manager.cancel_on_signal().await.unwrap();
}

#[derive(Default)]
struct Handler {
    count: AtomicUsize,
}

#[async_trait]
impl RequestHandler for Handler {
    type Req = usize;

    type Res = usize;

    async fn handle_request(
        &self,
        request: Self::Req,
        _cancellation_token: CancellationToken,
    ) -> Self::Res {
        // Print to stderr so we don't interfere with the transport
        eprintln!("Ping {request}");
        self.count.fetch_add(1, Ordering::SeqCst) + 1
    }
}
