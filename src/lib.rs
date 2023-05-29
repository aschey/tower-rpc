use async_trait::async_trait;

mod codec;
pub use codec::*;
#[cfg(feature = "server")]
mod request_handler;
#[cfg(feature = "server")]
pub use request_handler::*;
#[cfg(feature = "server")]
mod server;
mod service;
#[cfg(feature = "server")]
pub use server::*;
#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use client::*;
#[cfg(feature = "multiplex")]
mod tagged;
#[cfg(feature = "multiplex")]
pub use tagged::*;
mod request;
pub mod transport;
pub use request::*;
#[cfg(feature = "router")]
mod router;
#[cfg(feature = "router")]
pub use router::*;

use tower::{Service, ServiceExt};

mod private {
    pub trait Sealed {}
}

pub trait ServerMode: private::Sealed {}

pub struct Pipeline;
impl private::Sealed for Pipeline {}
impl ServerMode for Pipeline {}

#[cfg(feature = "multiplex")]
pub struct Multiplex {}
#[cfg(feature = "multiplex")]
impl private::Sealed for Multiplex {}
#[cfg(feature = "multiplex")]
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
