use crate::{Keyed, Request, RoutedRequest};
use async_trait::async_trait;
use background_service::{error::BoxedError, BackgroundService, ServiceContext};
use bytes::{Bytes, BytesMut};
use futures::{Future, Stream};
use futures_cancel::FutureExt;
use http::Method;
use http_body_util::{BodyExt, Full};
use std::{error::Error, fmt::Debug, marker::PhantomData, pin::Pin, task::Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::{Deserializer, Serializer};
use tokio_stream::StreamExt;
use tower::{BoxError, MakeService, Service, ServiceBuilder};

pub struct Server<K, H, S, I, E, Res>
where
    K: MakeService<(), hyper::Request<hyper::body::Incoming>, Service = H>,
    H: Service<hyper::Request<hyper::body::Incoming>, Response = http::Response<Res>>
        + Send
        + 'static,
{
    incoming: S,
    handler: K,

    _phantom: PhantomData<(H, I, E)>,
}

impl<K, H, S, I, E, Res> Server<K, H, S, I, E, Res>
where
    K: MakeService<(), hyper::Request<hyper::body::Incoming>, Service = H> + Send,
    K::Future: Send,
    K::MakeError: Debug,
    H: Service<hyper::Request<hyper::body::Incoming>, Response = http::Response<Res>>
        + Send
        + 'static,
    H::Future: Send + 'static,
    H::Error: Into<Box<dyn Error + Send + Sync>>,
    S: Stream<Item = Result<I, E>> + Send,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    E: Send,
    Res: hyper::body::Body + Send + 'static,
    <Res as hyper::body::Body>::Error: std::error::Error + Send + Sync,
    <Res as hyper::body::Body>::Data: std::marker::Send,
{
    pub fn new(incoming: S, handler: K) -> Self {
        Self {
            incoming,
            handler,
            _phantom: Default::default(),
        }
    }

    async fn run_server(mut self, mut context: ServiceContext) -> Result<(), BoxedError> {
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

            context
                .add_service((
                    "http_handler".to_owned(),
                    move |_: ServiceContext| async move {
                        let service = ServiceBuilder::default()
                            .layer_fn(|inner| ServiceWrapper {
                                inner,
                                _phantom: Default::default(),
                            })
                            .service(handler);

                        hyper::server::conn::http1::Builder::new()
                            .serve_connection(stream, service)
                            .await
                            .unwrap();

                        Ok(())
                    },
                ))
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl<K, H, S, I, E, Res> BackgroundService for Server<K, H, S, I, E, Res>
where
    K: MakeService<(), hyper::Request<hyper::body::Incoming>, Service = H> + Send,
    K::Future: Send,
    K::MakeError: Debug,
    H: Service<hyper::Request<hyper::body::Incoming>, Response = http::Response<Res>>
        + Send
        + 'static,
    H::Future: Send + 'static,
    H::Error: Into<Box<dyn Error + Send + Sync>>,
    S: Stream<Item = Result<I, E>> + Send,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    E: Send,
    Res: hyper::body::Body + Send + 'static,
    <Res as hyper::body::Body>::Error: std::error::Error + Send + Sync,
    <Res as hyper::body::Body>::Data: std::marker::Send,
{
    fn name(&self) -> &str {
        "rpc_server"
    }

    async fn run(mut self, context: ServiceContext) -> Result<(), BoxedError> {
        self.run_server(context).await
    }
}

pub struct HttpAdapter<S, D, M, Req>
where
    S: Service<Request<RoutedRequest<Req, Keyed<M>>>> + Clone + Send,
    M: From<Method>,
{
    inner: S,
    _phantom: PhantomData<(M, Req)>,
    serializer: D,
    context: ServiceContext,
}

impl<S, D, M, Req> HttpAdapter<S, D, M, Req>
where
    S: Service<Request<RoutedRequest<Req, Keyed<M>>>> + Clone + Send,
    M: From<Method>,
{
    pub fn new(inner: S, serializer: D, context: ServiceContext) -> Self {
        Self {
            inner,
            _phantom: Default::default(),
            serializer,
            context,
        }
    }
}

impl<S, D, M, Req> Service<hyper::Request<hyper::body::Incoming>> for HttpAdapter<S, D, M, Req>
where
    Req: Send,
    S: Service<Request<RoutedRequest<Req, Keyed<M>>>> + Clone + Send + 'static,
    S::Future: Send,
    S::Error: std::fmt::Debug,
    D: Serializer<S::Response>
        + Deserializer<Req>
        + Clone
        + Unpin
        + std::fmt::Debug
        + Send
        + 'static,
    <D as Serializer<S::Response>>::Error: std::fmt::Debug + Send + 'static,
    <D as Deserializer<Req>>::Error: std::fmt::Debug + Send + 'static,
    M: From<Method> + Send,
{
    type Error = BoxError;
    type Response = http::Response<Full<Bytes>>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: hyper::Request<hyper::body::Incoming>) -> Self::Future {
        let mut inner = self.inner.clone();
        let mut deser = self.serializer.clone();
        let context = self.context.clone();
        Box::pin(async move {
            let res = inner
                .call(Request {
                    context,
                    value: RoutedRequest {
                        route: req.uri().to_string(),
                        key: req.method().to_owned().into(),
                        value: Pin::new(&mut deser)
                            .deserialize(&BytesMut::from(
                                req.into_body()
                                    .collect()
                                    .await
                                    .unwrap()
                                    .to_bytes()
                                    .to_vec()
                                    .as_slice(),
                            ))
                            .unwrap(),
                    },
                })
                .await
                .unwrap();
            Ok(hyper::Response::builder()
                .body(Full::new(Pin::new(&mut deser).serialize(&res).unwrap()))
                .unwrap())
        })
    }
}

struct ServiceWrapper<S, Req>
where
    S: Service<hyper::Request<Req>>,
{
    inner: S,
    _phantom: PhantomData<Req>,
}

impl<S, Req, Res> Service<hyper::Request<Req>> for ServiceWrapper<S, Req>
where
    S: Service<hyper::Request<Req>, Response = http::Response<Res>>,
{
    type Error = S::Error;
    type Response = S::Response;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: hyper::Request<Req>) -> Self::Future {
        self.inner.call(req)
    }
}

impl<S, Req, Res> hyper::service::Service<hyper::Request<Req>> for ServiceWrapper<S, Req>
where
    S: Service<hyper::Request<Req>, Response = http::Response<Res>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, req: hyper::Request<Req>) -> Self::Future {
        tower::Service::call(self, req)
    }
}

impl<S, D, M, Req> hyper::service::Service<hyper::Request<hyper::body::Incoming>>
    for HttpAdapter<S, D, M, Req>
where
    Req: Send,
    S: Service<Request<RoutedRequest<Req, Keyed<M>>>> + Clone + Send + 'static,
    S::Future: Send,
    S::Error: std::fmt::Debug,
    D: Serializer<S::Response>
        + Deserializer<Req>
        + Clone
        + Unpin
        + std::fmt::Debug
        + Send
        + 'static,
    <D as Serializer<S::Response>>::Error: std::fmt::Debug + Send + 'static,
    <D as Deserializer<Req>>::Error: std::fmt::Debug + Send + 'static,
    M: From<Method> + Send,
{
    type Response = http::Response<Full<Bytes>>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&mut self, req: http::Request<hyper::body::Incoming>) -> Self::Future {
        tower::Service::call(self, req)
    }
}
