use futures::{Sink, TryStream};
use std::{fmt::Debug, marker::PhantomData};
use tokio_tower::pipeline;

#[cfg(feature = "multiplex")]
mod multiplex;

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
