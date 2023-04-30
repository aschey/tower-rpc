use async_trait::async_trait;

mod codec;
pub use codec::*;
mod request_handler;
pub use request_handler::*;
mod rpc_service;
mod server;
pub use server::*;
mod client;
pub use client::*;
mod tagged;
pub use tagged::*;
mod request;
pub mod transport;
pub use request::*;
mod router;
pub use router::*;
use tower::{Service, ServiceExt};

mod private {
    pub trait Sealed {}
}

pub trait ServerMode: private::Sealed {}

pub struct Pipeline;
impl private::Sealed for Pipeline {}
impl ServerMode for Pipeline {}

pub struct Multiplex {}
impl private::Sealed for Multiplex {}
impl ServerMode for Multiplex {}

#[async_trait]
pub trait ReadyServiceExt<Request>: Service<Request>
where
    Request: Send,
{
    async fn call_ready(&mut self, request: Request) -> Result<Self::Response, Self::Error>;
}

#[async_trait]
impl<S, Request> ReadyServiceExt<Request> for S
where
    Request: Send + 'static,
    S::Future: Send,
    S::Error: Send,
    S: Service<Request> + Send,
{
    async fn call_ready(&mut self, request: Request) -> Result<Self::Response, Self::Error> {
        self.ready().await?.call(request).await
    }
}
