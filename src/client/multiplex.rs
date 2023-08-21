use std::fmt::Debug;
use std::marker::PhantomData;
use std::pin::Pin;

use futures::{Sink, TryStream};
use tokio_tower::multiplex::{self, MultiplexTransport, TagStore};
use tower::util::BoxService;
use tower::{ServiceBuilder, ServiceExt};

use crate::service::DemultiplexService;
use crate::{Client, ClientError, Tagged};

#[cfg(feature = "multiplex")]
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

        ServiceBuilder::default()
            .layer_fn(DemultiplexService::new)
            .service(client)
            .boxed()
    }
}

pub(crate) struct SlabStore<Req, Res> {
    slab: slab::Slab<()>,
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
