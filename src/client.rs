use std::{fmt::Debug, marker::PhantomData, pin::Pin};

use futures::{Sink, TryStream};
use slab::Slab;
use tokio_tower::{
    multiplex::{self, MultiplexTransport, TagStore},
    pipeline,
};
use tower::{util::BoxService, ServiceBuilder};

use crate::{rpc_service::DemultiplexService, Tagged};

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

impl<S, Req, Res> Client<S, Tagged<Req>, Tagged<Res>>
where
    S: TryStream<Ok = Tagged<Res>> + Sink<Tagged<Req>> + Send + 'static,
    <S as futures::TryStream>::Error: Debug,
    <S as futures::Sink<Tagged<Req>>>::Error: Debug,
    Req: Unpin + Send + 'static,
    Res: Unpin + Send + Sync + Debug + 'static,
{
    pub fn create_multiplex(self) -> BoxService<Req, Res, ClientError> {
        let client =
            multiplex::Client::builder(MultiplexTransport::new(self.stream, SlabStore::default()))
                .build();

        BoxService::new(
            ServiceBuilder::default()
                .layer_fn(DemultiplexService::new)
                .service(client),
        )
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

pub(crate) struct SlabStore<Req, Res> {
    slab: Slab<()>,
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req, Res> Default for SlabStore<Req, Res> {
    fn default() -> Self {
        Self {
            slab: Default::default(),
            _phantom: Default::default(),
        }
    }
}

impl<Req, Res> TagStore<Tagged<Req>, Tagged<Res>> for SlabStore<Req, Res>
where
    Req: Unpin,
    Res: Unpin,
{
    type Tag = usize;
    fn assign_tag(mut self: Pin<&mut Self>, request: &mut Tagged<Req>) -> usize {
        let tag = self.slab.insert(());
        request.tag = tag;
        tag
    }

    fn finish_tag(mut self: Pin<&mut Self>, response: &Tagged<Res>) -> usize {
        self.slab.remove(response.tag);
        response.tag
    }
}
