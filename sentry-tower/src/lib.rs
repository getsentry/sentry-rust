//! Adds support for automatic hub binding for each request received by the Tower server (or client,
//! though usefulness is limited in this case).
//!
//! This allows breadcrumbs collected during the request handling to land in a specific hub, and
//! avoid having them mixed across requests should a new hub be bound at each request.
//!
//! # Examples
//!
//! ```rust
//! # use tower::ServiceBuilder;
//! # use std::time::Duration;
//! # type Request = String;
//! use sentry_tower::NewSentryLayer;
//!
//! // Compose a Tower service where each request gets its own Sentry hub
//! let service = ServiceBuilder::new()
//!     .layer(NewSentryLayer::<Request>::new_from_top())
//!     .timeout(Duration::from_secs(30))
//!     .service(tower::service_fn(|req: Request| format!("hello {}", req)));
//! ```
//!
//! More customization can be achieved through the `new` function, such as passing a [`Hub`]
//! directly.
//!
//! ```rust
//! # use tower::ServiceBuilder;
//! # use std::{sync::Arc, time::Duration};
//! # type Request = String;
//! use sentry::Hub;
//! use sentry_tower::SentryLayer;
//!
//! // Create a hub dedicated to web requests
//! let hub = Arc::new(Hub::with(|hub| Hub::new_from_top(hub)));
//!
//! // Compose a Tower service
//! let service = ServiceBuilder::new()
//!     .layer(SentryLayer::<_, _, Request>::new(hub))
//!     .timeout(Duration::from_secs(30))
//!     .service(tower::service_fn(|req: Request| format!("hello {}", req)));
//! ```
//!
//! The layer can also accept a closure to return a hub depending on the incoming request.
//!
//! ```rust
//! # use tower::ServiceBuilder;
//! # use std::{sync::Arc, time::Duration};
//! # type Request = String;
//! use sentry::Hub;
//! use sentry_tower::SentryLayer;
//!
//! // Compose a Tower service
//! let hello = Arc::new(Hub::with(|hub| Hub::new_from_top(hub)));
//! let other = Arc::new(Hub::with(|hub| Hub::new_from_top(hub)));
//!
//! let service = ServiceBuilder::new()
//!     .layer(SentryLayer::new(|req: &Request| match req.as_str() {
//!         "hello" => hello.clone(),
//!         _ => other.clone(),
//!     }))
//!     .timeout(Duration::from_secs(30))
//!     .service(tower::service_fn(|req: Request| format!("{} world", req)));
//! ```
//!
//! When using Tonic, the layer can be used directly by the Tonic stack:
//!
//! ```rust,no_run
//! # use anyhow::{anyhow, Result};
//! # use sentry_anyhow::capture_anyhow;
//! # use tonic::{Request, Response, Status, transport::Server};
//! # mod hello_world {
//! #     include!("helloworld.rs");
//! # }
//! use hello_world::{greeter_server::*, *};
//! use sentry_tower::NewSentryLayer;
//!
//! struct GreeterService;
//!
//! #[tonic::async_trait]
//! impl Greeter for GreeterService {
//!     async fn say_hello(
//!         &self,
//!         req: Request<HelloRequest>,
//!     ) -> Result<Response<HelloReply>, Status> {
//!         let HelloRequest { name } = req.into_inner();
//!         if name == "world" {
//!             capture_anyhow(&anyhow!("Trying to greet a planet"));
//!             return Err(Status::invalid_argument("Cannot greet a planet"));
//!         }
//!         Ok(Response::new(HelloReply {
//!             message: format!("Hello {}", name),
//!         }))
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<()> {
//! Server::builder()
//!     .layer(NewSentryLayer::new_from_top())
//!     .add_service(GreeterServer::new(GreeterService))
//!     .serve("127.0.0.1:50051".parse().unwrap())
//!     .await?;
//! #     Ok(())
//! # }
//! ```
//!
//! ## Usage with `tower-http`
//!
//! The `http` feature of the `sentry-tower` crate offers another layer which will attach
//! request details onto captured events, and optionally start a new performance monitoring
//! transaction based on the incoming HTTP headers.  When using the tower integration via
//! `sentry::integrations::tower`, this feature can also be enabled using the `tower-http`
//! feature of the `sentry` crate instead of the `tower` feature.
//!
//! The created transaction will automatically use the request URI as its name.
//! This is sometimes not desirable in case the request URI contains unique IDs
//! or similar. In this case, users should manually override the transaction name
//! in the request handler using the [`Scope::set_transaction`](sentry_core::Scope::set_transaction)
//! method.
//!
//! When combining both layers, take care of the ordering of both. For example
//! with [`tower::ServiceBuilder`], always define the `Hub` layer before the `Http`
//! one, like so:
//!
//! ```rust
//! # #[cfg(feature = "http")] {
//! # type Request = http::Request<String>;
//! let layer = tower::ServiceBuilder::new()
//!     .layer(sentry_tower::NewSentryLayer::<Request>::new_from_top())
//!     .layer(sentry_tower::SentryHttpLayer::new().enable_transaction());
//! # }
//! ```
//!
//! When using `axum`, either use [`tower::ServiceBuilder`] as shown above, or make sure you
//! reorder the layers, like so:
//!
//! ```rust
//! let app = Router::new()
//!     .route("/", get(handler))
//!     .layer(sentry_tower::SentryHttpLayer::with_transaction())
//!     .layer(sentry_tower::NewSentryLayer::<Request>::new_from_top())
//! ```
//!
//! This is because `axum` applies middleware in the opposite order as [`tower::ServiceBuilder`].
//! Applying the layers in the wrong order can result in memory leaks.
//!
//! [`tower::ServiceBuilder`]: https://docs.rs/tower/latest/tower/struct.ServiceBuilder.html

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

use std::marker::PhantomData;
use std::sync::Arc;
use std::task::{Context, Poll};

use sentry_core::{Hub, SentryFuture, SentryFutureExt};
use tower_layer::Layer;
use tower_service::Service;

#[cfg(feature = "http")]
mod http;
#[cfg(feature = "http")]
pub use crate::http::*;

/// Provides a hub for each request
pub trait HubProvider<H, Request>
where
    H: Into<Arc<Hub>>,
{
    /// Returns a hub to be bound to the request
    fn hub(&self, request: &Request) -> H;
}

impl<H, F, Request> HubProvider<H, Request> for F
where
    F: Fn(&Request) -> H,
    H: Into<Arc<Hub>>,
{
    fn hub(&self, request: &Request) -> H {
        (self)(request)
    }
}

impl<Request> HubProvider<Arc<Hub>, Request> for Arc<Hub> {
    fn hub(&self, _request: &Request) -> Arc<Hub> {
        self.clone()
    }
}

/// Provides a new hub made from the currently active hub for each request
#[derive(Clone, Copy)]
pub struct NewFromTopProvider;

impl<Request> HubProvider<Arc<Hub>, Request> for NewFromTopProvider {
    fn hub(&self, _request: &Request) -> Arc<Hub> {
        // The Clippy lint here is a false positive, the suggestion to write
        // `Hub::with(Hub::new_from_top)` does not compiles:
        //     143 |         Hub::with(Hub::new_from_top).into()
        //         |         ^^^^^^^^^ implementation of `std::ops::FnOnce` is not general enough
        #[allow(clippy::redundant_closure)]
        Hub::with(|hub| Hub::new_from_top(hub)).into()
    }
}

/// Tower layer that binds a specific Sentry hub for each request made.
pub struct SentryLayer<P, H, Request>
where
    P: HubProvider<H, Request>,
    H: Into<Arc<Hub>>,
{
    provider: P,
    _hub: PhantomData<(H, fn() -> Request)>,
}

impl<S, P, H, Request> Layer<S> for SentryLayer<P, H, Request>
where
    P: HubProvider<H, Request> + Clone,
    H: Into<Arc<Hub>>,
{
    type Service = SentryService<S, P, H, Request>;

    fn layer(&self, service: S) -> Self::Service {
        SentryService {
            service,
            provider: self.provider.clone(),
            _hub: PhantomData,
        }
    }
}

impl<P, H, Request> Clone for SentryLayer<P, H, Request>
where
    P: HubProvider<H, Request> + Clone,
    H: Into<Arc<Hub>>,
{
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            _hub: PhantomData,
        }
    }
}

impl<P, H, Request> SentryLayer<P, H, Request>
where
    P: HubProvider<H, Request> + Clone,
    H: Into<Arc<Hub>>,
{
    /// Build a new layer with the given Layer provider
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            _hub: PhantomData,
        }
    }
}

/// Tower service that binds a specific Sentry hub for each request made.
pub struct SentryService<S, P, H, Request>
where
    P: HubProvider<H, Request>,
    H: Into<Arc<Hub>>,
{
    service: S,
    provider: P,
    _hub: PhantomData<(H, fn() -> Request)>,
}

impl<S, Request, P, H> Service<Request> for SentryService<S, P, H, Request>
where
    S: Service<Request>,
    P: HubProvider<H, Request>,
    H: Into<Arc<Hub>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = SentryFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let hub = self.provider.hub(&request).into();
        let fut = Hub::run(hub.clone(), || self.service.call(request));
        fut.bind_hub(hub)
    }
}

impl<S, P, H, Request> Clone for SentryService<S, P, H, Request>
where
    S: Clone,
    P: HubProvider<H, Request> + Clone,
    H: Into<Arc<Hub>>,
{
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            provider: self.provider.clone(),
            _hub: PhantomData,
        }
    }
}

impl<S, P, H, Request> SentryService<S, P, H, Request>
where
    P: HubProvider<H, Request> + Clone,
    H: Into<Arc<Hub>>,
{
    /// Wrap a Tower service with a Tower layer that binds a Sentry hub for each request made.
    pub fn new(provider: P, service: S) -> Self {
        SentryLayer::<P, H, Request>::new(provider).layer(service)
    }
}

/// Tower layer that binds a new Sentry hub for each request made
pub type NewSentryLayer<Request> = SentryLayer<NewFromTopProvider, Arc<Hub>, Request>;

impl<Request> NewSentryLayer<Request> {
    /// Create a new Sentry layer that binds a new Sentry hub for each request made
    pub fn new_from_top() -> Self {
        Self {
            provider: NewFromTopProvider,
            _hub: PhantomData,
        }
    }
}

/// Tower service that binds a new Sentry hub for each request made.
pub type NewSentryService<S, Request> = SentryService<S, NewFromTopProvider, Arc<Hub>, Request>;

impl<S, Request> NewSentryService<S, Request> {
    /// Wrap a Tower service with a Tower layer that binds a Sentry hub for each request made.
    pub fn new_from_top(service: S) -> Self {
        Self {
            provider: NewFromTopProvider,
            service,
            _hub: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_layer_is_sync_when_request_isnt() {
        assert_sync::<NewSentryLayer<Rc<()>>>(); // Rc<()> is not Sync
    }

    #[test]
    fn test_service_is_sync_when_request_isnt() {
        assert_sync::<NewSentryService<(), Rc<()>>>(); // Rc<()> is not Sync
    }
}
