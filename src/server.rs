use std::{fmt::Debug, marker::PhantomData};

use crate::{rpc_service::RequestService, Pipeline, Request, ServerMode};
use async_trait::async_trait;
use background_service::{error::BoxedError, BackgroundService, ServiceContext};
use futures::{Sink, Stream, TryStream};
use futures_cancel::FutureExt;
use tokio_stream::StreamExt;
use tokio_tower::pipeline;
use tower::{MakeService, ServiceBuilder};

pub struct Server<K, H, S, I, E, M, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H>,
    H: tower::Service<Request<Req>, Response = Res>,
    S: Stream<Item = Result<I, E>>,
    M: ServerMode,
{
    incoming: S,
    handler: K,
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
            context
                .add_service((
                    "ipc_handler".to_owned(),
                    move |context: ServiceContext| async move {
                        let service = ServiceBuilder::default()
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

#[cfg(feature = "multiplex")]
impl<K, H, S, I, E, Req, Res> Server<K, H, S, I, E, crate::Multiplex, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H>,
    K::MakeError: Debug,
    H: tower::Service<Request<Req>, Response = Res> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Send + Debug,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = crate::Tagged<Req>> + Sink<crate::Tagged<Res>> + Send + 'static,
    <I as futures::TryStream>::Error: Debug,
    <I as futures::Sink<crate::Tagged<Res>>>::Error: Debug,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn multiplex(incoming: S, handler: K) -> Self {
        Self {
            incoming,
            handler,
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
            context
                .add_service((
                    "ipc_handler".to_owned(),
                    move |context: ServiceContext| async move {
                        let service = ServiceBuilder::default()
                            .layer_fn(crate::rpc_service::MultiplexService::new)
                            .layer_fn(|inner| RequestService::new(context.clone(), inner))
                            .service(handler);
                        tokio_tower::multiplex::Server::new(stream, service)
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

#[cfg(feature = "multiplex")]
#[async_trait]
impl<K, H, S, I, E, Req, Res> BackgroundService
    for Server<K, H, S, I, E, crate::Multiplex, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H> + Send,
    K::MakeError: Debug,
    K::Future: Send,
    H: tower::Service<Request<Req>, Response = Res> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Send + Debug,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = crate::Tagged<Req>> + Sink<crate::Tagged<Res>> + Send + 'static,
    <I as futures::TryStream>::Error: Debug,
    <I as futures::Sink<crate::Tagged<Res>>>::Error: Debug,
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
