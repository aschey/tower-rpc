use std::sync::Arc;

use crate::{rpc_service::RpcService, CodecBuilder, RequestHandler, ServerMode};
use async_trait::async_trait;
use background_service::{error::BoxedError, BackgroundService, ServiceContext};
use futures::Stream;
use futures_cancel::FutureExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_stream::StreamExt;
use tokio_tower::{multiplex, pipeline};
use tower::{
    layer::util::{Identity, Stack},
    ServiceBuilder,
};

pub struct Server<H, T, S, I, E, Req, Res, L = Identity>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    T: CodecBuilder<Req = Req, Res = Res>,
    S: Stream<Item = Result<I, E>>,
    I: AsyncRead + AsyncWrite,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    incoming: S,
    handler: Arc<H>,
    codec_builder: T,
    service_builder: ServiceBuilder<L>,
    server_mode: ServerMode,
}

impl<H, T, S, I, E, Req, Res> Server<H, T, S, I, E, Req, Res>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    T: CodecBuilder<Req = Req, Res = Res>,
    S: Stream<Item = Result<I, E>> + Send,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn new(incoming: S, handler: H, codec_builder: T, server_mode: ServerMode) -> Self {
        Self {
            incoming,
            handler: Arc::new(handler),
            codec_builder,
            server_mode,
            service_builder: ServiceBuilder::new(),
        }
    }
}

impl<H, T, S, I, E, Req, Res, L> Server<H, T, S, I, E, Req, Res, L>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    T: CodecBuilder<Req = Req, Res = Res>,
    S: Stream<Item = Result<I, E>> + Send,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn layer<NewLayer>(
        self,
        layer: NewLayer,
    ) -> Server<H, T, S, I, E, Req, Res, Stack<NewLayer, L>> {
        Server {
            service_builder: self.service_builder.layer(layer),
            codec_builder: self.codec_builder,
            handler: self.handler,
            incoming: self.incoming,
            server_mode: self.server_mode,
        }
    }
}

#[async_trait]
impl<H, T, S, I, E, Req, Res> BackgroundService for Server<H, T, S, I, E, Req, Res>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    T: CodecBuilder<Req = Req, Res = Res>,
    S: Stream<Item = Result<I, E>> + Send,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "ipc_server"
    }
    async fn run(self, mut context: ServiceContext) -> Result<(), BoxedError> {
        let incoming = self.incoming;
        futures::pin_mut!(incoming);
        while let Ok(Some(Ok(stream))) = incoming
            .next()
            .cancel_on_shutdown(&context.cancellation_token())
            .await
        {
            let transport = self.codec_builder.build_codec(stream);
            let handler = self.handler.clone();
            let service_builder = self.service_builder.clone();
            let server_mode = self.server_mode;
            context
                .add_service((
                    "ipc_handler".to_owned(),
                    move |context: ServiceContext| async move {
                        let service = service_builder.service(RpcService::new(
                            handler.clone(),
                            context.cancellation_token(),
                        ));
                        match server_mode {
                            ServerMode::Pipeline => {
                                pipeline::Server::new(transport, service)
                                    .cancel_on_shutdown(&context.cancellation_token())
                                    .await
                                    .unwrap()
                                    .unwrap();
                            }
                            ServerMode::Multiplex => {
                                multiplex::Server::new(transport, service)
                                    .cancel_on_shutdown(&context.cancellation_token())
                                    .await
                                    .unwrap()
                                    .unwrap();
                            }
                        }

                        Ok(())
                    },
                ))
                .await
                .unwrap();
        }
        Ok(())
    }
}
