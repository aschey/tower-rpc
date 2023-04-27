use background_service::BackgroundServiceManager;

use futures::StreamExt;

use tokio_util::{codec::LinesCodec, sync::CancellationToken};

use tower_rpc::{
    codec_builder_fn,
    transport::{stdio::StdioTransport, CodecTransport},
    RequestHandlerStream, Server,
};

#[tokio::main]
pub async fn main() {
    let cancellation_token = CancellationToken::default();
    let manager = BackgroundServiceManager::new(cancellation_token.clone());

    let mut stream = RequestHandlerStream::default();
    let mut handler = stream.request_stream().unwrap();

    let server = Server::pipeline(
        CodecTransport::new(
            StdioTransport::incoming(),
            codec_builder_fn(|s| {
                Box::new(tokio_util::codec::Framed::new(s, LinesCodec::default()))
            }),
        ),
        stream,
    );
    let mut context = manager.get_context();
    context.add_service(server).await.unwrap();
    while let Some((req, res)) = handler.next().await {
        dbg!(req);
        res.respond("test".to_owned()).unwrap();
    }
}
