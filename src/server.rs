use std::sync::Arc;

use crate::{
    get_socket_address, ipc_request_handler::IpcRequestHandler, ipc_service::IpcService,
    ServerMode, TransportBuilder,
};
use async_trait::async_trait;
use background_service::{error::BoxedError, BackgroundService, ServiceContext};
use futures_cancel::FutureExt;
use parity_tokio_ipc::Endpoint;
use tokio_stream::StreamExt;
use tokio_tower::{multiplex, pipeline};
use tower::{
    layer::util::{Identity, Stack},
    ServiceBuilder,
};

pub struct Server<H, T, Req, Res, L = Identity>
where
    H: IpcRequestHandler<Req = Req, Res = Res> + 'static,
    T: TransportBuilder<Req = Req, Res = Res>,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    endpoint: Endpoint,
    handler: Arc<H>,
    transport_builder: T,
    service_builder: ServiceBuilder<L>,
    server_mode: ServerMode,
}

impl<H, T, Req, Res> Server<H, T, Req, Res>
where
    H: IpcRequestHandler<Req = Req, Res = Res> + 'static,
    T: TransportBuilder<Req = Req, Res = Res>,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn new(
        app_id: impl Into<String>,
        handler: H,
        transport_builder: T,
        server_mode: ServerMode,
    ) -> Self {
        let app_id = app_id.into();
        let endpoint = Endpoint::new(get_socket_address(&app_id, ""));

        Self {
            endpoint,
            handler: Arc::new(handler),
            transport_builder,
            server_mode,
            service_builder: ServiceBuilder::new(),
        }
    }
}

impl<H, T, Req, Res, L> Server<H, T, Req, Res, L>
where
    H: IpcRequestHandler<Req = Req, Res = Res> + 'static,
    T: TransportBuilder<Req = Req, Res = Res>,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn layer<NewLayer>(self, layer: NewLayer) -> Server<H, T, Req, Res, Stack<NewLayer, L>> {
        Server {
            service_builder: self.service_builder.layer(layer),
            transport_builder: self.transport_builder,
            handler: self.handler,
            endpoint: self.endpoint,
            server_mode: self.server_mode,
        }
    }
}

#[async_trait]
impl<H, T, Req, Res> BackgroundService for Server<H, T, Req, Res>
where
    H: IpcRequestHandler<Req = Req, Res = Res> + 'static,
    T: TransportBuilder<Req = Req, Res = Res>,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "ipc_server"
    }
    async fn run(self, mut context: ServiceContext) -> Result<(), BoxedError> {
        let incoming = self.endpoint.incoming().expect("failed to open socket");
        futures::pin_mut!(incoming);
        while let Ok(Some(Ok(stream))) = incoming
            .next()
            .cancel_on_shutdown(&context.cancellation_token())
            .await
        {
            let transport = self.transport_builder.build_transport(stream);
            let handler = self.handler.clone();
            let service_builder = self.service_builder.clone();
            let server_mode = self.server_mode;
            context
                .add_service((
                    "ipc_handler".to_owned(),
                    move |context: ServiceContext| async move {
                        let service = service_builder.service(IpcService::new(
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
