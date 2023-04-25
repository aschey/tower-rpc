use std::{fmt::Debug, marker::PhantomData, pin::Pin};

use futures::{Sink, TryStream};
use slab::Slab;
use tokio_tower::{
    multiplex::{self, MultiplexTransport, TagStore},
    pipeline,
};

use crate::{get_socket_address, ipc_client_stream::IpcClientStream, Tagged, TransportBuilder};

pub struct Client<T, Req, Res>
where
    T: TransportBuilder<Req = Req, Res = Res> + 'static,
    Req: Send,
    Res: Send,
{
    transport_builder: T,
    app_id: String,
}

impl<T, Req, Res> Client<T, Req, Res>
where
    T: TransportBuilder<Req = Req, Res = Res> + 'static,
    Req: Send + 'static,
    Res: Send + 'static,
{
    pub fn new(app_id: impl Into<String>, transport_builder: T) -> Self {
        Self {
            app_id: app_id.into(),
            transport_builder,
        }
    }
    pub fn create_pipeline(&self) -> impl tower::Service<Res, Error = ClientError, Response = Req> {
        let stream = IpcClientStream::new(get_socket_address(&self.app_id, ""));
        let transport = self.transport_builder.build_transport(stream);
        let client: pipeline::Client<_, ClientError, _> = pipeline::Client::new(transport);
        client
    }
}

impl<T, Req, Res> Client<T, Tagged<Req>, Tagged<Res>>
where
    T: TransportBuilder<Req = Tagged<Req>, Res = Tagged<Res>> + 'static,
    Req: Unpin + Send + 'static,
    Res: Unpin + Send + 'static,
{
    pub fn create_multiplex(&self) -> impl tower::Service<Tagged<Res>> {
        let stream = IpcClientStream::new(get_socket_address(&self.app_id, ""));

        let transport = self.transport_builder.build_transport(stream);
        let client: multiplex::Client<_, ClientError, _> =
            multiplex::Client::builder(MultiplexTransport::new(transport, SlabStore::default()))
                .build();
        client
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
