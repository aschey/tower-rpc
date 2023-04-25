use std::{fmt::Debug, marker::PhantomData, pin::Pin};

use futures::{Sink, TryStream};
use slab::Slab;
use tokio_tower::{
    multiplex::{self, MultiplexTransport, TagStore},
    pipeline,
};
use tower::{
    layer::util::{Identity, Stack},
    ServiceBuilder,
};

use crate::Tagged;

pub struct Client<S, Req, Res, L = Identity> {
    stream: S,
    service_builder: ServiceBuilder<L>,
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
            service_builder: ServiceBuilder::default(),
            _phantom: Default::default(),
        }
    }

    pub fn create_pipeline(self) -> impl tower::Service<Req, Error = ClientError, Response = Res> {
        let client = pipeline::Client::new(self.stream);
        self.service_builder.service(client)
    }
}

impl<S, Req, Res, L> Client<S, Req, Res, L> {
    pub fn layer<NewLayer>(self, layer: NewLayer) -> Client<S, Req, Res, Stack<NewLayer, L>> {
        Client {
            stream: self.stream,
            service_builder: self.service_builder.layer(layer),
            _phantom: Default::default(),
        }
    }
}

impl<S, Req, Res> Client<S, Tagged<Req>, Tagged<Res>>
where
    S: TryStream<Ok = Tagged<Res>> + Sink<Tagged<Req>> + Send + 'static,
    <S as futures::TryStream>::Error: Debug,
    <S as futures::Sink<Tagged<Req>>>::Error: Debug,
    Req: Unpin + Send + 'static,
    Res: Unpin + Send + 'static,
{
    pub fn create_multiplex(
        self,
    ) -> impl tower::Service<Tagged<Req>, Error = ClientError, Response = Tagged<Res>> {
        let client =
            multiplex::Client::builder(MultiplexTransport::new(self.stream, SlabStore::default()))
                .build();
        self.service_builder.service(client)
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
