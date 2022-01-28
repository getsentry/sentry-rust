//! The provided transports.
//!
//! This module exposes all transports that are compiled into the sentry
//! library.  The `reqwest`, `curl`, `surf` and `ureq` features turn on these transports.

use crate::{ClientOptions, Transport, TransportFactory};
use std::sync::Arc;

#[cfg(feature = "httpdate")]
mod ratelimit;
#[cfg(any(feature = "curl", feature = "ureq"))]
mod thread;
#[cfg(any(feature = "reqwest", feature = "surf",))]
mod tokio_thread;

#[cfg(feature = "reqwest")]
mod reqwest;
#[cfg(feature = "reqwest")]
pub use reqwest::ReqwestHttpTransport;

#[cfg(feature = "curl")]
mod curl;
#[cfg(feature = "curl")]
pub use curl::CurlHttpTransport;

#[cfg(feature = "surf")]
mod surf;
#[cfg(feature = "surf")]
pub use surf::SurfHttpTransport;

#[cfg(feature = "ureq")]
mod ureq;
#[cfg(feature = "ureq")]
pub use ureq::UreqHttpTransport;

#[cfg(feature = "reqwest")]
type DefaultTransport = ReqwestHttpTransport;

#[cfg(all(
    feature = "curl",
    not(feature = "reqwest"),
    not(feature = "surf"),
    not(feature = "ureq")
))]
type DefaultTransport = CurlHttpTransport;

#[cfg(all(
    feature = "surf",
    not(feature = "reqwest"),
    not(feature = "curl"),
    not(feature = "ureq")
))]
type DefaultTransport = SurfHttpTransport;

#[cfg(all(
    feature = "ureq",
    not(feature = "reqwest"),
    not(feature = "curl"),
    not(feature = "surf")
))]
type DefaultTransport = UreqHttpTransport;

/// The default http transport.
#[cfg(any(
    feature = "reqwest",
    feature = "curl",
    feature = "surf",
    feature = "ureq"
))]
pub type HttpTransport = DefaultTransport;

/// Creates the default HTTP transport.
///
/// This is the default value for `transport` on the client options.  It
/// creates a `HttpTransport`.  If no http transport was compiled into the
/// library it will panic on transport creation.
#[derive(Clone)]
pub struct DefaultTransportFactory;

impl TransportFactory for DefaultTransportFactory {
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport> {
        #[cfg(any(
            feature = "reqwest",
            feature = "curl",
            feature = "surf",
            feature = "ureq"
        ))]
        {
            Arc::new(HttpTransport::new(options))
        }
        #[cfg(not(any(
            feature = "reqwest",
            feature = "curl",
            feature = "surf",
            feature = "ureq"
        )))]
        {
            let _ = options;
            panic!("sentry crate was compiled without transport")
        }
    }
}
