use std::fmt::Debug;
use std::marker::PhantomData;

use async_trait::async_trait;
use background_service::error::BoxedError;
use background_service::{BackgroundService, ServiceContext};
use futures::{Sink, Stream, TryStream};
use futures_cancel::FutureExt;
use tokio_stream::StreamExt;
use tokio_tower::pipeline;
use tower::{MakeService, ServiceBuilder};
use tracing::info;

use crate::service::RequestService;
use crate::{Pipeline, Request, ServerMode};

#[cfg(feature = "multiplex")]
mod multiplex;
#[cfg(feature = "multiplex")]
pub use multiplex::*;

#[cfg(feature = "http")]
pub mod http;

pub struct Server<K, H, S, I, E, M, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H>,
    H: tower::Service<Request<Req>, Response = Res>,
    S: Stream<Item = Result<I, E>>,
    M: ServerMode,
{
    pub(super) incoming: S,
    pub(super) handler: K,
    pub(super) _phantom: PhantomData<(M, H, Req, Res)>,
}

impl<K, H, S, I, E, Req, Res> Server<K, H, S, I, E, Pipeline, Req, Res>
where
    K: MakeService<(), Request<Req>, Service = H>,
    K::MakeError: Debug,
    H: tower::Service<Request<Req>, Response = Res> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Debug + Send,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = Req> + Sink<Res> + Send + 'static,
    <I as TryStream>::Error: Debug,
    <I as Sink<Res>>::Error: Debug,
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

    async fn run_pipeline(mut self, mut context: ServiceContext) -> Result<(), BoxedError> {
        let incoming = self.incoming;
        futures::pin_mut!(incoming);
        while let Ok(Some(Ok(stream))) = incoming
            .next()
            .cancel_on_shutdown(&context.cancellation_token())
            .await
        {
            let handler = self
                .handler
                .make_service(())
                .await
                .map_err(|e| format!("{e:?}"))?;
            context.add_service((
                "rpc_handler".to_owned(),
                move |context: ServiceContext| async move {
                    let service = ServiceBuilder::default()
                        .layer_fn(|inner| RequestService::new(context.clone(), inner))
                        .service(handler);

                    if let Ok(res) = pipeline::Server::new(stream, service)
                        .cancel_on_shutdown(&context.cancellation_token())
                        .await
                    {
                        return match res {
                            Ok(()) => Ok(()),
                            Err(pipeline::server::Error::Service(e)) => Err(format!("{e:?}"))?,
                            Err(e) => {
                                // Transport errors can happen if the client disconnects so they may
                                // be expected
                                info!("Transport failure: {e:?}");
                                Ok(())
                            }
                        };
                    }

                    Ok(())
                },
            ));
        }

        Ok(())
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
    H::Error: Debug + Send,
    S: Stream<Item = Result<I, E>> + Send,
    I: TryStream<Ok = Req> + Sink<Res> + Send + 'static,
    <I as TryStream>::Error: Debug,
    <I as Sink<Res>>::Error: Debug,
    E: Send,
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "rpc_server"
    }

    async fn run(mut self, context: ServiceContext) -> Result<(), BoxedError> {
        self.run_pipeline(context).await
    }
}
