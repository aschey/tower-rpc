use std::error::Error;
use std::fmt::Debug;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use async_trait::async_trait;
use background_service::error::BoxedError;
use background_service::{BackgroundService, ServiceContext};
use bytes::{Bytes, BytesMut};
use eyre::Context as EyreContext;
use futures::{Future, Stream};
use futures_cancel::FutureExt;
use http::Method;
use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Incoming};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::{Deserializer, Serializer};
use tokio_stream::StreamExt;
use tower::{BoxError, MakeService, Service, ServiceBuilder};
use tracing::error;

use crate::{Keyed, Request, RoutedRequest};

pub struct Server<K, H, S, I, E, Res>
where
    K: MakeService<(), hyper::Request<Incoming>, Service = H>,
    H: Service<hyper::Request<Incoming>, Response = http::Response<Res>> + Send + 'static,
{
    incoming: S,
    handler: K,

    _phantom: PhantomData<(H, I, E)>,
}

impl<K, H, S, I, E, Res> Server<K, H, S, I, E, Res>
where
    K: MakeService<(), hyper::Request<Incoming>, Service = H> + Send,
    K::MakeError: Error + Send + Sync + 'static,
    K::Future: Send,
    K::MakeError: Debug,
    H: Service<hyper::Request<Incoming>, Response = http::Response<Res>> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Into<Box<dyn Error + Send + Sync>>,
    S: Stream<Item = Result<I, E>> + Send,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    E: Send,
    Res: Body + Send + 'static,
    <Res as Body>::Error: Error + Send + Sync,
    <Res as Body>::Data: Send,
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
                .wrap_err("Error making service")?;

            context.add_service((
                "http_handler".to_owned(),
                move |context: ServiceContext| async move {
                    let service = ServiceBuilder::default()
                        .layer_fn(|inner| ServiceWrapper {
                            inner,
                            _phantom: Default::default(),
                        })
                        .service(Arc::new(Mutex::new(handler)));

                    if let Ok(Err(e)) =
                        hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                            .serve_connection(TokioIo::new(stream), service)
                            .cancel_on_shutdown(&context.cancellation_token())
                            .await
                    {
                        error!("Error serving connection: {e:?}");
                    }

                    Ok(())
                },
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl<K, H, S, I, E, Res> BackgroundService for Server<K, H, S, I, E, Res>
where
    K: MakeService<(), hyper::Request<Incoming>, Service = H> + Send,
    K::MakeError: Error + Send + Sync + 'static,
    K::Future: Send,
    K::MakeError: Debug,
    H: Service<hyper::Request<Incoming>, Response = http::Response<Res>> + Send + 'static,
    H::Future: Send + 'static,
    H::Error: Into<Box<dyn Error + Send + Sync>>,
    S: Stream<Item = Result<I, E>> + Send,
    I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    E: Send,
    Res: Body + Send + 'static,
    <Res as Body>::Error: Error + Send + Sync,
    <Res as Body>::Data: Send,
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

impl<S, D, M, Req> Service<hyper::Request<Incoming>> for HttpAdapter<S, D, M, Req>
where
    Req: Send,
    S: Service<Request<RoutedRequest<Req, Keyed<M>>>, Error = BoxError> + Clone + Send + 'static,
    S::Future: Send,
    D: Serializer<S::Response, Error = io::Error>
        + Deserializer<Req, Error = io::Error>
        + Clone
        + Unpin
        + Debug
        + Send
        + 'static,
    M: From<Method> + Send,
{
    type Error = BoxError;
    type Response = http::Response<Full<Bytes>>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: hyper::Request<Incoming>) -> Self::Future {
        hyper::service::Service::call(self, req)
    }
}

struct ServiceWrapper<S, Req>
where
    S: Service<hyper::Request<Req>>,
{
    inner: Arc<Mutex<S>>,
    _phantom: PhantomData<Req>,
}

impl<S, Req, Res> Service<hyper::Request<Req>> for ServiceWrapper<S, Req>
where
    S: Service<hyper::Request<Req>, Response = http::Response<Res>>,
{
    type Error = S::Error;
    type Response = S::Response;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.lock().expect("lock poisoned").poll_ready(cx)
    }

    fn call(&mut self, req: hyper::Request<Req>) -> Self::Future {
        self.inner.lock().expect("lock poisoned").call(req)
    }
}

impl<S, Req, Res> hyper::service::Service<hyper::Request<Req>> for ServiceWrapper<S, Req>
where
    S: Service<hyper::Request<Req>, Response = http::Response<Res>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&self, req: hyper::Request<Req>) -> Self::Future {
        self.inner.lock().expect("lock poisoned").call(req)
    }
}

impl<S, D, M, Req> hyper::service::Service<hyper::Request<Incoming>> for HttpAdapter<S, D, M, Req>
where
    Req: Send,
    S: Service<Request<RoutedRequest<Req, Keyed<M>>>, Error = BoxError> + Clone + Send + 'static,
    S::Future: Send,
    D: Serializer<S::Response, Error = io::Error>
        + Deserializer<Req, Error = io::Error>
        + Clone
        + Unpin
        + Debug
        + Send
        + 'static,
    M: From<Method> + Send,
{
    type Response = http::Response<Full<Bytes>>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: http::Request<hyper::body::Incoming>) -> Self::Future {
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
                                    .wrap_err("Error collecting request body")?
                                    .to_bytes()
                                    .to_vec()
                                    .as_slice(),
                            ))
                            .wrap_err("Error deserializing request")?,
                    },
                })
                .await?;
            Ok(hyper::Response::builder()
                .body(Full::new(
                    Pin::new(&mut deser)
                        .serialize(&res)
                        .wrap_err("Error serializing response")?,
                ))
                .wrap_err("Error creating response body")?)
        })
    }
}
