use std::{fmt::Debug, marker::PhantomData, pin::Pin};

use futures::{Sink, TryStream};
use tokio_tower::{
    multiplex::{self, MultiplexTransport, TagStore},
    pipeline,
};
use tower::{util::BoxService, ServiceBuilder, ServiceExt};

pub struct Client<S, Req, Res> {
    stream: S,
    _phantom: PhantomData<(Req, Res)>,
}

impl<S, Req, Res> Client<S, Req, Res>
where
    S: TryStream<Ok = Res> + Sink<Req> + Send + 'static,
    <S as futures::TryStream>::Error: Debug,
    <S as futures::Sink<Req>>::Error: Debug,
    Req: Send + 'static,
    Res: Send + 'static,
{
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            _phantom: Default::default(),
        }
    }

    pub fn create_pipeline(self) -> pipeline::client::Client<S, ClientError, Req> {
        pipeline::Client::new(self.stream)
    }
}

#[cfg(feature = "multiplex")]
impl<S, Req, Res> Client<S, crate::Tagged<Req>, crate::Tagged<Res>>
where
    S: TryStream<Ok = crate::Tagged<Res>> + Sink<crate::Tagged<Req>> + Send + 'static,
    <S as futures::TryStream>::Error: Debug,
    <S as futures::Sink<crate::Tagged<Req>>>::Error: Debug,
    Req: Unpin + Send + 'static,
    Res: Unpin + Send + Sync + Debug + 'static,
{
    pub fn create_multiplex(self) -> BoxService<Req, Res, ClientError> {
        let client =
            multiplex::Client::builder(MultiplexTransport::new(self.stream, SlabStore::default()))
                .build();

        ServiceBuilder::default()
            .layer_fn(crate::rpc_service::DemultiplexService::new)
            .service(client)
            .boxed()
    }
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct ClientError(String);
impl<T, I> From<tokio_tower::Error<T, I>> for ClientError
where
    T: Sink<I> + TryStream,
    <T as Sink<I>>::Error: Debug,
    <T as futures::TryStream>::Error: Debug,
{
    fn from(e: tokio_tower::Error<T, I>) -> Self {
        Self(format!("{e:?}"))
    }
}

#[cfg(feature = "multiplex")]
pub(crate) struct SlabStore<Req, Res> {
    slab: slab::Slab<()>,
    _phantom: PhantomData<(Req, Res)>,
}

#[cfg(feature = "multiplex")]
impl<Req, Res> Default for SlabStore<Req, Res> {
    fn default() -> Self {
        Self {
            slab: Default::default(),
            _phantom: Default::default(),
        }
    }
}

#[cfg(feature = "multiplex")]
impl<Req, Res> TagStore<crate::Tagged<Req>, crate::Tagged<Res>> for SlabStore<Req, Res>
where
    Req: Unpin,
    Res: Unpin,
{
    type Tag = usize;
    fn assign_tag(mut self: Pin<&mut Self>, request: &mut crate::Tagged<Req>) -> usize {
        let tag = self.slab.insert(());
        request.tag = tag;
        tag
    }

    fn finish_tag(mut self: Pin<&mut Self>, response: &crate::Tagged<Res>) -> usize {
        self.slab.remove(response.tag);
        response.tag
    }
}
