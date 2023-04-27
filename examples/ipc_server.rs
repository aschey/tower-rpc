use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use background_service::BackgroundServiceManager;

use tokio_util::sync::CancellationToken;

use tower_rpc::{
    transport::{ipc, CodecTransport},
    Codec, RequestHandler, SerdeCodec, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());
    let transport = ipc::Transport::new("test");

    let server = Server::pipeline(
        CodecTransport::new(
            transport.incoming().unwrap(),
            SerdeCodec::<usize, usize>::new(Codec::Bincode),
        ),
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
        println!("Ping {request}");
        self.count.fetch_add(1, Ordering::SeqCst) + 1
    }
}
