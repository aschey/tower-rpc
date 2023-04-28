use std::{fmt::Debug, marker::PhantomData};

use crate::{
    rpc_service::{MultiplexService, RequestService},
    Multiplex, Pipeline, Request, ServerMode, Tagged,
};
use async_trait::async_trait;
use background_service::{error::BoxedError, BackgroundService, ServiceContext};
use futures::{Sink, Stream, TryStream};
use futures_cancel::FutureExt;
use tokio_stream::StreamExt;
use tokio_tower::{multiplex, pipeline};
use tower::{
    layer::util::{Identity, Stack},
    MakeService, ServiceBuilder,
};

pub struct Server<K, H, S, I, E, M, Req, Res, L = Identity>
where
    K: MakeService<(), Request<Req>, Service = H>,
    H: tower::Service<Request<Req>, Response = Res>,
    S: Stream<Item = Result<I, E>>,
    M: ServerMode,
{
    incoming: S,
    handler: K,
    service_builder: ServiceBuilder<L>,
    _phantom: PhantomData<(M, H, Req, Res)>,
}

impl<K, H, S, I, E, Req, Res> Server<K, H, S, I, E, Pipeline, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H>,
    K::MakeError: Debug,
    H: tower::Service<Request<Req>, Response = Res> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Send + Debug,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = Req> + Sink<Res> + Send + 'static,
    <I as futures::TryStream>::Error: Debug,
    <I as futures::Sink<Res>>::Error: Debug,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn pipeline(incoming: S, handler: K) -> Self {
        Self {
            incoming,
            handler,
            service_builder: ServiceBuilder::new(),
            _phantom: Default::default(),
        }
    }

    async fn run_pipeline(mut self, mut context: ServiceContext) {
        let incoming = self.incoming;
        futures::pin_mut!(incoming);
        while let Ok(Some(Ok(stream))) = incoming
            .next()
            .cancel_on_shutdown(&context.cancellation_token())
            .await
        {
            let handler = self.handler.make_service(()).await.unwrap();
            let service_builder = self.service_builder.clone();

            context
                .add_service((
                    "ipc_handler".to_owned(),
                    move |context: ServiceContext| async move {
                        let service = service_builder
                            .layer_fn(|inner| RequestService::new(context.clone(), inner))
                            .service(handler);
                        pipeline::Server::new(stream, service)
                            .cancel_on_shutdown(&context.cancellation_token())
                            .await
                            .unwrap()
                            .unwrap();

                        Ok(())
                    },
                ))
                .await
                .unwrap();
        }
    }
}

impl<K, H, S, I, E, Req, Res> Server<K, H, S, I, E, Multiplex, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H>,
    K::MakeError: Debug,
    H: tower::Service<Request<Req>, Response = Res> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Send + Debug,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = Tagged<Req>> + Sink<Tagged<Res>> + Send + 'static,
    <I as futures::TryStream>::Error: Debug,
    <I as futures::Sink<Tagged<Res>>>::Error: Debug,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn multiplex(incoming: S, handler: K) -> Self {
        Self {
            incoming,
            handler,
            service_builder: ServiceBuilder::new(),
            _phantom: Default::default(),
        }
    }

    async fn run_multiplex(mut self, mut context: ServiceContext) {
        let incoming = self.incoming;
        futures::pin_mut!(incoming);
        while let Ok(Some(Ok(stream))) = incoming
            .next()
            .cancel_on_shutdown(&context.cancellation_token())
            .await
        {
            let handler = self.handler.make_service(()).await.unwrap();
            let service_builder = self.service_builder.clone();

            context
                .add_service((
                    "ipc_handler".to_owned(),
                    move |context: ServiceContext| async move {
                        let service = service_builder
                            .layer_fn(MultiplexService::new)
                            .layer_fn(|inner| RequestService::new(context.clone(), inner))
                            .service(handler);
                        multiplex::Server::new(stream, service)
                            .cancel_on_shutdown(&context.cancellation_token())
                            .await
                            .unwrap()
                            .unwrap();

                        Ok(())
                    },
                ))
                .await
                .unwrap();
        }
    }
}

impl<K, H, S, I, E, M, Req, Res, L> Server<K, H, S, I, E, M, Req, Res, L>
where
    K: MakeService<(), Request<Req>, Service = H>,
    H: tower::Service<Request<Req>, Response = Res>,
    S: Stream<Item = Result<I, E>>,
    M: ServerMode,
{
    pub fn layer<NewLayer>(
        self,
        layer: NewLayer,
    ) -> Server<K, H, S, I, E, M, Req, Res, Stack<NewLayer, L>> {
        Server {
            service_builder: self.service_builder.layer(layer),
            handler: self.handler,
            incoming: self.incoming,
            _phantom: self._phantom,
        }
    }
}

#[async_trait]
impl<K, H, S, I, E, Req, Res> BackgroundService for Server<K, H, S, I, E, Pipeline, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H> + Send,
    K::MakeError: Debug,
    K::Future: Send,
    H: tower::Service<Request<Req>, Response = Res> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Send + Debug,
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
    async fn run(mut self, context: ServiceContext) -> Result<(), BoxedError> {
        self.run_pipeline(context).await;
        Ok(())
    }
}

#[async_trait]
impl<K, H, S, I, E, Req, Res> BackgroundService for Server<K, H, S, I, E, Multiplex, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H> + Send,
    K::MakeError: Debug,
    K::Future: Send,
    H: tower::Service<Request<Req>, Response = Res> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Send + Debug,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = Tagged<Req>> + Sink<Tagged<Res>> + Send + 'static,
    <I as futures::TryStream>::Error: Debug,
    <I as futures::Sink<Tagged<Res>>>::Error: Debug,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "ipc_server"
    }
    async fn run(self, context: ServiceContext) -> Result<(), BoxedError> {
        self.run_multiplex(context).await;
        Ok(())
    }
}
