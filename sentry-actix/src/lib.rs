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
#![allow(clippy::needless_doctest_main)]
//! ```no_run
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
//!
//!     server::new(|| {
//!         App::new()
//!             .middleware(SentryMiddleware::new())
//!             .resource("/", |r| r.f(failing))
//!     }).bind("127.0.0.1:3001")
//!         .unwrap()
//!         .run();
//! }
//! ```
//!
//! # Reusing the Hub
//!
//! If you use this integration the `Hub::current()` returned hub is typically the wrong one.
//! To get the request specific one you need to use the `ActixWebHubExt` trait:
//!
//! ```
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

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use actix_web::middleware::{Finished, Middleware, Response, Started};
use actix_web::{Error, HttpMessage, HttpRequest, HttpResponse};
use failure::Fail;
use sentry::protocol::{ClientSdkPackage, Event, Level};
use sentry::types::Uuid;
use sentry::{Hub, ScopeGuard};
use sentry_failure::exception_from_single_fail;

/// A helper construct that can be used to reconfigure and build the middleware.
pub struct SentryMiddlewareBuilder {
    middleware: SentryMiddleware,
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

/// Reports certain failures to sentry.
pub struct SentryMiddleware {
    hub: Option<Arc<Hub>>,
    emit_header: bool,
    capture_server_errors: bool,
}

struct HubWrapper {
    hub: Arc<Hub>,
    root_scope: RefCell<Option<ScopeGuard>>,
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
        Arc::new(Hub::new_from_top(Hub::main()))
    }
}

impl Default for SentryMiddleware {
    fn default() -> Self {
        SentryMiddleware::new()
    }
}

fn extract_request<S: 'static>(
    req: &HttpRequest<S>,
    with_pii: bool,
) -> (Option<String>, sentry::protocol::Request) {
    let resource = req.resource();
    let transaction = if let Some(rdef) = resource.rdef() {
        Some(rdef.pattern().to_string())
    } else if resource.name() != "" {
        Some(resource.name().to_string())
    } else {
        None
    };
    let mut sentry_req = sentry::protocol::Request {
        url: format!(
            "{}://{}{}",
            req.connection_info().scheme(),
            req.connection_info().host(),
            req.uri()
        )
        .parse()
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

        let root_scope = hub.push_scope();
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
                    sdk.packages.push(ClientSdkPackage {
                        name: "sentry-actix".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                    });
                    event.sdk = Some(Cow::Owned(sdk));
                }

                Some(event)
            }));
        });

        outer_req.extensions_mut().insert(HubWrapper {
            hub,
            root_scope: RefCell::new(Some(root_scope)),
        });
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
                        event_id.to_simple_ref().to_string().parse().unwrap(),
                    );
                }
                _ => {}
            }
        }
        Ok(Response::Done(resp))
    }

    fn finish(&self, req: &HttpRequest<S>, _resp: &HttpResponse) -> Finished {
        // if we make it to the end of the request we want to first drop the root
        // scope before we drop the entire hub.  This will first drop the closures
        // on the scope which in turn will release the circular dependency we have
        // with the hub via the request.
        if let Some(hub_wrapper) = req.extensions().get::<HubWrapper>() {
            if let Ok(mut guard) = hub_wrapper.root_scope.try_borrow_mut() {
                guard.take();
            }
        }
        Finished::Done
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
            .get::<HubWrapper>()
            .expect("SentryMiddleware middleware was not registered")
            .hub
            .clone()
    }

    fn capture_actix_error(&self, err: &Error) -> Uuid {
        let mut exceptions = vec![];
        let mut ptr: Option<&dyn Fail> = Some(err.as_fail());
        let mut idx = 0;
        while let Some(fail) = ptr {
            // Check whether the failure::Fail held by err is a failure::Error wrapped in Compat
            // If that's the case, we should be logging that error and its fail instead of the wrapper's construction in actix_web
            // This wouldn't be necessary if failure::Compat<failure::Error>'s Fail::backtrace() impl was not "|| None",
            // that is however impossible to do as of now because it conflicts with the generic implementation of Fail also provided in failure.
            // Waiting for update that allows overlap, (https://github.com/rust-lang/rfcs/issues/1053), but chances are by then failure/std::error will be refactored anyway
            let compat: Option<&failure::Compat<failure::Error>> = fail.downcast_ref();
            let failure_err = compat.map(failure::Compat::get_ref);
            let fail = failure_err.map_or(fail, |x| x.as_fail());
            exceptions.push(exception_from_single_fail(
                fail,
                if idx == 0 {
                    Some(failure_err.map_or_else(|| err.backtrace(), |err| err.backtrace()))
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
