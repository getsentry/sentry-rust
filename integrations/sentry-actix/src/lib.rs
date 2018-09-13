//! This crate adds a middleware for [`actix-web`](https://actix.rs/) that captures errors and
//! report them to `Sentry`.
//!
//! To use this middleware just configure Sentry and then add it to your actix web app as a
//! middleware.  Because actix is generally working with non sendable objects and highly concurrent
//! this middleware creates a new hub per request.  As a result many of the sentry integrations
//! such as breadcrumbs do not work unless you bind the actix hub.
//!
//! # Example
//!
//! ```
//! extern crate actix_web;
//! extern crate sentry;
//! extern crate sentry_actix;
//!
//! # fn main() {
//! use std::env;
//! use std::io;
//!
//! use actix_web::{server, App, Error, HttpRequest};
//! use sentry_actix::SentryMiddleware;
//!
//! fn failing(_req: &HttpRequest) -> Result<String, Error> {
//!     Err(io::Error::new(io::ErrorKind::Other, "An error happens here").into())
//! }
//!
//! fn main() {
//!     let _guard = sentry::init("https://public@sentry.io/1234");
//!     env::set_var("RUST_BACKTRACE", "1");
//!     sentry::integrations::panic::register_panic_handler();
//!
//!     server::new(|| {
//!         App::new()
//!             .middleware(SentryMiddleware::new())
//!             .resource("/", |r| r.f(failing))
//!     }).bind("127.0.0.1:3001")
//!         .unwrap()
//!         .run();
//! }
//! # }
//! ```
//!
//! # Reusing the Hub
//!
//! If you use this integration the `Hub::current()` returned hub is typically the wrong one.
//! To get the request specific one you need to use the `ActixWebHubExt` trait:
//!
//! ```
//! # extern crate sentry;
//! # extern crate sentry_actix;
//! # extern crate actix_web;
//! # fn test(req: &actix_web::HttpRequest) {
//! use sentry::{Hub, Level};
//! use sentry_actix::ActixWebHubExt;
//!
//! let hub = Hub::from_request(req);
//! hub.capture_message("Something is not well", Level::Warning);
//! # }
//! ```
//!
//! The hub can also be made current:
//!
//! ```
//! # extern crate sentry;
//! # extern crate sentry_actix;
//! # extern crate actix_web;
//! # fn test(req: &actix_web::HttpRequest) {
//! use sentry::{Hub, Level};
//! use sentry_actix::ActixWebHubExt;
//!
//! let hub = Hub::from_request(req);
//! Hub::run(hub, || {
//!     sentry::capture_message("Something is not well", Level::Warning);
//! });
//! # }
//! ```
extern crate actix_web;
extern crate failure;
extern crate fragile;
extern crate sentry;
extern crate uuid;

use actix_web::middleware::{Middleware, Response, Started};
use actix_web::{Error, HttpMessage, HttpRequest, HttpResponse};
use failure::Fail;
use sentry::integrations::failure::exception_from_single_fail;
use sentry::protocol::{ClientSdkPackageInfo, Event, Level};
use sentry::Hub;
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A helper construct that can be used to reconfigure and build the middleware.
pub struct SentryMiddlewareBuilder {
    middleware: SentryMiddleware,
}

/// Reports certain failures to sentry.
pub struct SentryMiddleware {
    hub: Option<Arc<Hub>>,
    emit_header: bool,
    capture_server_errors: bool,
}

impl SentryMiddlewareBuilder {
    /// Finishes the building and returns a middleware
    pub fn finish(self) -> SentryMiddleware {
        self.middleware
    }

    /// Reconfigures the middleware so that it uses a specific hub instead of the default one.
    pub fn with_hub(mut self, hub: Arc<Hub>) -> Self {
        self.middleware.hub = Some(hub);
        self
    }

    /// Reconfigures the middleware so that it uses a specific hub instead of the default one.
    pub fn with_default_hub(mut self) -> Self {
        self.middleware.hub = None;
        self
    }

    /// If configured the sentry id is attached to a X-Sentry-Event header.
    pub fn emit_header(mut self, val: bool) -> Self {
        self.middleware.emit_header = val;
        self
    }

    /// Enables or disables error reporting.
    ///
    /// The default is to report all errors.
    pub fn capture_server_errors(mut self, val: bool) -> Self {
        self.middleware.capture_server_errors = val;
        self
    }
}

impl SentryMiddleware {
    /// Creates a new sentry middleware.
    pub fn new() -> SentryMiddleware {
        SentryMiddleware {
            hub: None,
            emit_header: false,
            capture_server_errors: true,
        }
    }

    /// Creates a new middleware builder.
    pub fn builder() -> SentryMiddlewareBuilder {
        SentryMiddleware::new().into_builder()
    }

    /// Converts the middleware into a builder.
    pub fn into_builder(self) -> SentryMiddlewareBuilder {
        SentryMiddlewareBuilder { middleware: self }
    }

    fn new_hub(&self) -> Arc<Hub> {
        Arc::new(Hub::new_from_top(Hub::current()))
    }
}

fn extract_request<S: 'static>(
    req: &HttpRequest<S>,
    with_pii: bool,
) -> (Option<String>, sentry::protocol::Request) {
    let resource = req.resource();
    let transaction = if let Some(rdef) = resource.rdef() {
        Some(rdef.pattern().to_string())
    } else {
        if resource.name() != "" {
            Some(resource.name().to_string())
        } else {
            None
        }
    };
    let mut sentry_req = sentry::protocol::Request {
        url: format!(
            "{}://{}{}",
            req.connection_info().scheme(),
            req.connection_info().host(),
            req.uri()
        ).parse()
        .ok(),
        method: Some(req.method().to_string()),
        headers: req
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().into(), v.to_str().unwrap_or("").into()))
            .collect(),
        ..Default::default()
    };

    if with_pii {
        if let Some(remote) = req.connection_info().remote() {
            sentry_req.env.insert("REMOTE_ADDR".into(), remote.into());
        }
    };

    (transaction, sentry_req)
}

impl<S: 'static> Middleware<S> for SentryMiddleware {
    fn start(&self, req: &HttpRequest<S>) -> Result<Started, Error> {
        let hub = self.new_hub();
        let outer_req = req;
        let req = outer_req.clone();
        let client = hub.client();

        let req = fragile::SemiSticky::new(req);
        let cached_data = Arc::new(Mutex::new(None));

        hub.configure_scope(move |scope| {
            scope.add_event_processor(Box::new(move |mut event| {
                let mut cached_data = cached_data.lock().unwrap();
                if cached_data.is_none() && req.is_valid() {
                    let with_pii = client
                        .as_ref()
                        .map_or(false, |x| x.options().send_default_pii);
                    *cached_data = Some(extract_request(&req.get(), with_pii));
                }

                if let Some((ref transaction, ref req)) = *cached_data {
                    if event.transaction.is_none() {
                        event.transaction = transaction.clone();
                    }
                    if event.request.is_none() {
                        event.request = Some(req.clone());
                    }
                }

                if let Some(sdk) = event.sdk.take() {
                    let mut sdk = sdk.into_owned();
                    sdk.packages.push(ClientSdkPackageInfo {
                        package_name: "sentry-actix".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                    });
                    event.sdk = Some(Cow::Owned(sdk));
                }

                Some(event)
            }));
        });

        outer_req.extensions_mut().insert(hub);
        Ok(Started::Done)
    }

    fn response(&self, req: &HttpRequest<S>, mut resp: HttpResponse) -> Result<Response, Error> {
        if self.capture_server_errors && resp.status().is_server_error() {
            let event_id = if let Some(error) = resp.error() {
                Some(Hub::from_request(req).capture_actix_error(error))
            } else {
                None
            };
            match event_id {
                Some(event_id) if self.emit_header => {
                    resp.headers_mut().insert(
                        "x-sentry-event",
                        event_id.simple().to_string().parse().unwrap(),
                    );
                }
                _ => {}
            }
        }
        Ok(Response::Done(resp))
    }
}

/// Utility function that takes an actix error and reports it to the default hub.
///
/// This is typically not very since the actix hub is likely never bound as the
/// default hub.  It's generally recommended to use the `ActixWebHubExt` trait's
/// extension method on the hub instead.
pub fn capture_actix_error(err: &Error) -> Uuid {
    Hub::with_active(|hub| hub.capture_actix_error(err))
}

/// Hub extensions for actix.
pub trait ActixWebHubExt {
    /// Returns the hub from a given http request.
    ///
    /// This requires that the `SentryMiddleware` middleware has been enabled or the
    /// call will panic.
    fn from_request<S>(req: &HttpRequest<S>) -> Arc<Hub>;
    /// Captures an actix error on the given hub.
    fn capture_actix_error(&self, err: &Error) -> Uuid;
}

impl ActixWebHubExt for Hub {
    fn from_request<S>(req: &HttpRequest<S>) -> Arc<Hub> {
        req.extensions()
            .get::<Arc<Hub>>()
            .expect("SentryMiddleware middleware was not registered")
            .clone()
    }

    fn capture_actix_error(&self, err: &Error) -> Uuid {
        let mut exceptions = vec![];
        let mut ptr: Option<&Fail> = Some(err.as_fail());
        let mut idx = 0;
        while let Some(fail) = ptr {
            exceptions.push(exception_from_single_fail(
                fail,
                if idx == 0 {
                    Some(err.backtrace())
                } else {
                    fail.backtrace()
                },
            ));
            ptr = fail.cause();
            idx += 1;
        }
        exceptions.reverse();
        self.capture_event(Event {
            exception: exceptions.into(),
            level: Level::Error,
            ..Default::default()
        })
    }
}
