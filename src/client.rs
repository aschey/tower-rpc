use std::{fmt::Debug, marker::PhantomData, pin::Pin};

use futures::{Sink, TryStream};
use slab::Slab;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tower::{
    multiplex::{self, MultiplexTransport, TagStore},
    pipeline,
};
use tower::{
    layer::util::{Identity, Stack},
    ServiceBuilder,
};

use crate::{CodecBuilder, Tagged};

pub struct Client<T, S, Req, Res, L = Identity>
where
    T: CodecBuilder<Req = Req, Res = Res> + 'static,
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    Req: Send,
    Res: Send,
{
    transport_builder: T,
    stream: S,
    service_builder: ServiceBuilder<L>,
}

impl<T, S, Req, Res> Client<T, S, Req, Res>
where
    T: CodecBuilder<Req = Req, Res = Res> + 'static,
    S: AsyncRead + AsyncWrite + Send + Unpin,
    Req: Send + 'static,
    Res: Send + 'static,
{
    pub fn new(stream: S, transport_builder: T) -> Self {
        Self {
            stream,
            transport_builder,
            service_builder: ServiceBuilder::default(),
        }
    }

    pub fn create_pipeline(self) -> impl tower::Service<Res, Error = ClientError, Response = Req> {
        let transport = self.transport_builder.build_codec(self.stream);
        let client = pipeline::Client::new(transport);
        self.service_builder.service(client)
    }
}

impl<T, S, Req, Res, L> Client<T, S, Req, Res, L>
where
    T: CodecBuilder<Req = Req, Res = Res> + 'static,
    S: AsyncRead + AsyncWrite + Send + Unpin,
    Req: Send + 'static,
    Res: Send + 'static,
{
    pub fn layer<NewLayer>(self, layer: NewLayer) -> Client<T, S, Req, Res, Stack<NewLayer, L>> {
        Client {
            transport_builder: self.transport_builder,
            stream: self.stream,
            service_builder: self.service_builder.layer(layer),
        }
    }
}

impl<T, S, Req, Res> Client<T, S, Tagged<Req>, Tagged<Res>>
where
    T: CodecBuilder<Req = Tagged<Req>, Res = Tagged<Res>> + 'static,
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    Req: Unpin + Send + 'static,
    Res: Unpin + Send + 'static,
{
    pub fn create_multiplex(
        self,
    ) -> impl tower::Service<Tagged<Res>, Error = ClientError, Response = Tagged<Req>> {
        let transport = self.transport_builder.build_codec(self.stream);
        let client =
            multiplex::Client::builder(MultiplexTransport::new(transport, SlabStore::default()))
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
