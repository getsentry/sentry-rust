//! The provided transports.
//!
//! This module exposes all transports that are compiled into the sentry
//! library.  The `reqwest`, `curl`, and `ureq` features turn on these transports.

use crate::{ClientOptions, Transport, TransportFactory};
use std::sync::Arc;

#[cfg(feature = "httpdate")]
mod ratelimit;
#[cfg(feature = "httpdate")]
pub use self::ratelimit::{RateLimiter, RateLimitingCategory};

#[cfg(any(feature = "curl", feature = "ureq"))]
mod thread;
#[cfg(any(feature = "curl", feature = "ureq"))]
pub use self::thread::TransportThread as StdTransportThread;

#[cfg(feature = "reqwest")]
mod tokio_thread;
#[cfg(feature = "reqwest")]
pub use self::tokio_thread::TransportThread as TokioTransportThread;

#[cfg(feature = "reqwest")]
mod reqwest;
#[cfg(feature = "reqwest")]
pub use self::reqwest::ReqwestHttpTransport;

#[cfg(all(target_os = "espidf", feature = "embedded-svc-http"))]
mod embedded_svc_http;
#[cfg(all(target_os = "espidf", feature = "embedded-svc-http"))]
pub use self::embedded_svc_http::EmbeddedSVCHttpTransport;

#[cfg(feature = "curl")]
mod curl;
#[cfg(feature = "curl")]
pub use self::curl::CurlHttpTransport;

#[cfg(feature = "ureq")]
mod ureq;
#[cfg(feature = "ureq")]
pub use self::ureq::UreqHttpTransport;

#[cfg(feature = "reqwest")]
type DefaultTransport = ReqwestHttpTransport;

#[cfg(all(
    feature = "curl",
    not(all(target_os = "espidf", feature = "embedded-svc-http")),
    not(feature = "reqwest"),
    not(feature = "ureq")
))]
type DefaultTransport = CurlHttpTransport;

#[cfg(all(
    feature = "ureq",
    not(all(target_os = "espidf", feature = "embedded-svc-http")),
    not(feature = "reqwest"),
    not(feature = "curl"),
))]
type DefaultTransport = UreqHttpTransport;

#[cfg(all(
    target_os = "espidf",
    feature = "embedded-svc-http",
    not(feature = "reqwest"),
    not(feature = "curl"),
    not(feature = "ureq")
))]
type DefaultTransport = EmbeddedSVCHttpTransport;

/// The default http transport.
#[cfg(any(
    all(target_os = "espidf", feature = "embedded-svc-http"),
    feature = "reqwest",
    feature = "curl",
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
            all(target_os = "espidf", feature = "embedded-svc-http"),
            feature = "reqwest",
            feature = "curl",
            feature = "ureq"
        ))]
        {
            Arc::new(HttpTransport::new(options))
        }
        #[cfg(not(any(
            all(target_os = "espidf", feature = "embedded-svc-http"),
            feature = "reqwest",
            feature = "curl",
            feature = "ureq"
        )))]
        {
            let _ = options;
            panic!("sentry crate was compiled without transport")
        }
    }
}
