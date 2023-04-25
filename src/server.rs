use std::{fmt::Debug, sync::Arc};

use crate::{rpc_service::RpcService, RequestHandler, ServerMode};
use async_trait::async_trait;
use background_service::{error::BoxedError, BackgroundService, ServiceContext};
use futures::{Sink, Stream, TryStream};
use futures_cancel::FutureExt;
use tokio_stream::StreamExt;
use tokio_tower::{multiplex, pipeline};
use tower::{
    layer::util::{Identity, Stack},
    ServiceBuilder,
};

pub struct Server<H, S, I, E, Req, Res, L = Identity>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    S: Stream<Item = Result<I, E>>,
{
    incoming: S,
    handler: Arc<H>,
    service_builder: ServiceBuilder<L>,
    server_mode: ServerMode,
}

impl<H, S, I, E, Req, Res> Server<H, S, I, E, Req, Res>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    S: Stream<Item = Result<I, E>>,
{
    pub fn new(incoming: S, handler: H, server_mode: ServerMode) -> Self {
        Self {
            incoming,
            handler: Arc::new(handler),
            server_mode,
            service_builder: ServiceBuilder::new(),
        }
    }
}

impl<H, S, I, E, Req, Res, L> Server<H, S, I, E, Req, Res, L>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    S: Stream<Item = Result<I, E>>,
{
    pub fn layer<NewLayer>(
        self,
        layer: NewLayer,
    ) -> Server<H, S, I, E, Req, Res, Stack<NewLayer, L>> {
        Server {
            service_builder: self.service_builder.layer(layer),
            handler: self.handler,
            incoming: self.incoming,
            server_mode: self.server_mode,
        }
    }
}

#[async_trait]
impl<H, S, I, E, Req, Res> BackgroundService for Server<H, S, I, E, Req, Res>
where
    H: RequestHandler<Req = Req, Res = Res> + 'static,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = Req> + Sink<Res> + Send + 'static,
    <I as futures::TryStream>::Error: Debug,
    <I as futures::Sink<Res>>::Error: Debug,
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
                                pipeline::Server::new(stream, service)
                                    .cancel_on_shutdown(&context.cancellation_token())
                                    .await
                                    .unwrap()
                                    .unwrap();
                            }
                            ServerMode::Multiplex => {
                                multiplex::Server::new(stream, service)
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
