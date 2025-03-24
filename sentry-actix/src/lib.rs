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
//! ```no_run
//! use std::io;
//!
//! use actix_web::{get, App, Error, HttpRequest, HttpServer};
//!
//! #[get("/")]
//! async fn failing(_req: HttpRequest) -> Result<String, Error> {
//!     Err(io::Error::new(io::ErrorKind::Other, "An error happens here").into())
//! }
//!
//! fn main() -> io::Result<()> {
//!     let _guard = sentry::init(sentry::ClientOptions {
//!         release: sentry::release_name!(),
//!         ..Default::default()
//!     });
//!     std::env::set_var("RUST_BACKTRACE", "1");
//!
//!     let runtime = tokio::runtime::Builder::new_multi_thread()
//!         .enable_all()
//!         .build()?;
//!     runtime.block_on(async move {
//!         HttpServer::new(|| {
//!             App::new()
//!                 .wrap(sentry_actix::Sentry::new())
//!                 .service(failing)
//!         })
//!         .bind("127.0.0.1:3001")?
//!         .run()
//!         .await
//!     })
//! }
//! ```
//!
//! # Using Release Health
//!
//! The actix middleware will automatically start a new session for each request
//! when `auto_session_tracking` is enabled and the client is configured to
//! use `SessionMode::Request`.
//!
//! ```
//! let _sentry = sentry::init(sentry::ClientOptions {
//!     release: sentry::release_name!(),
//!     session_mode: sentry::SessionMode::Request,
//!     auto_session_tracking: true,
//!     ..Default::default()
//! });
//! ```
//!
//! # Reusing the Hub
//!
//! This integration will automatically create a new per-request Hub from the main Hub, and update the
//! current Hub instance. For example, the following will capture a message in the current request's Hub:
//!
//! ```
//! sentry::capture_message("Something is not well", sentry::Level::Warning);
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![allow(deprecated)]
#![allow(clippy::type_complexity)]

use std::borrow::Cow;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use actix_http::header::{self, HeaderMap};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::StatusCode;
use actix_web::Error;
use bytes::{Bytes, BytesMut};
use futures_util::future::{ok, Future, Ready};
use futures_util::{FutureExt as _, TryStreamExt as _};

use sentry_core::protocol::{self, ClientSdkPackage, Event, Request};
use sentry_core::MaxRequestBodySize;
use sentry_core::{Hub, SentryFutureExt};

/// A helper construct that can be used to reconfigure and build the middleware.
pub struct SentryBuilder {
    middleware: Sentry,
}

impl SentryBuilder {
    /// Finishes the building and returns a middleware
    pub fn finish(self) -> Sentry {
        self.middleware
    }

    /// Tells the middleware to start a new performance monitoring transaction for each request.
    #[must_use]
    pub fn start_transaction(mut self, start_transaction: bool) -> Self {
        self.middleware.start_transaction = start_transaction;
        self
    }

    /// Reconfigures the middleware so that it uses a specific hub instead of the default one.
    #[must_use]
    pub fn with_hub(mut self, hub: Arc<Hub>) -> Self {
        self.middleware.hub = Some(hub);
        self
    }

    /// Reconfigures the middleware so that it uses a specific hub instead of the default one.
    #[must_use]
    pub fn with_default_hub(mut self) -> Self {
        self.middleware.hub = None;
        self
    }

    /// If configured the sentry id is attached to a X-Sentry-Event header.
    #[must_use]
    pub fn emit_header(mut self, val: bool) -> Self {
        self.middleware.emit_header = val;
        self
    }

    /// Enables or disables error reporting.
    ///
    /// The default is to report all errors.
    #[must_use]
    pub fn capture_server_errors(mut self, val: bool) -> Self {
        self.middleware.capture_server_errors = val;
        self
    }
}

/// Reports certain failures to Sentry.
#[derive(Clone)]
pub struct Sentry {
    hub: Option<Arc<Hub>>,
    emit_header: bool,
    capture_server_errors: bool,
    start_transaction: bool,
}

impl Sentry {
    /// Creates a new sentry middleware.
    pub fn new() -> Self {
        Sentry {
            hub: None,
            emit_header: false,
            capture_server_errors: true,
            start_transaction: false,
        }
    }

    /// Creates a new sentry middleware which starts a new performance monitoring transaction for each request.
    pub fn with_transaction() -> Sentry {
        Sentry {
            start_transaction: true,
            ..Sentry::default()
        }
    }

    /// Creates a new middleware builder.
    pub fn builder() -> SentryBuilder {
        Sentry::new().into_builder()
    }

    /// Converts the middleware into a builder.
    pub fn into_builder(self) -> SentryBuilder {
        SentryBuilder { middleware: self }
    }
}

impl Default for Sentry {
    fn default() -> Self {
        Sentry::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for Sentry
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SentryMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(SentryMiddleware {
            service: Rc::new(service),
            inner: self.clone(),
        })
    }
}

/// The middleware for individual services.
pub struct SentryMiddleware<S> {
    service: Rc<S>,
    inner: Sentry,
}

fn should_capture_request_body(
    headers: &HeaderMap,
    max_request_body_size: MaxRequestBodySize,
) -> bool {
    let is_chunked = headers
        .get(header::TRANSFER_ENCODING)
        .and_then(|h| h.to_str().ok())
        .map(|transfer_encoding| transfer_encoding.contains("chunked"))
        .unwrap_or(false);

    let is_valid_content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|content_type| {
            matches!(
                content_type,
                "application/json" | "application/x-www-form-urlencoded"
            )
        });

    let is_within_size_limit = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|content_length| content_length.parse::<usize>().ok())
        .map(|content_length| max_request_body_size.is_within_size_limit(content_length))
        .unwrap_or(false);

    !is_chunked && is_valid_content_type && is_within_size_limit
}

/// Extract a body from the HTTP request
async fn body_from_http(req: &mut ServiceRequest) -> actix_web::Result<Bytes> {
    let stream = req.extract::<actix_web::web::Payload>().await?;
    let body = stream.try_collect::<BytesMut>().await?.freeze();

    // put copy of payload back into request for downstream to read
    req.set_payload(actix_web::dev::Payload::from(body.clone()));

    Ok(body)
}

async fn capture_request_body(req: &mut ServiceRequest) -> String {
    match body_from_http(req).await {
        Ok(request_body) => String::from_utf8_lossy(&request_body).into_owned(),
        Err(_) => String::new(),
    }
}

impl<S, B> Service<ServiceRequest> for SentryMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let inner = self.inner.clone();
        let hub = Arc::new(Hub::new_from_top(
            inner.hub.clone().unwrap_or_else(Hub::main),
        ));
        let client = hub.client();
        let track_sessions = client.as_ref().is_some_and(|client| {
            let options = client.options();
            options.auto_session_tracking
                && options.session_mode == sentry_core::SessionMode::Request
        });
        let max_request_body_size = client
            .as_ref()
            .map(|client| client.options().max_request_body_size)
            .unwrap_or(MaxRequestBodySize::None);
        if track_sessions {
            hub.start_session();
        }
        let with_pii = client
            .as_ref()
            .is_some_and(|client| client.options().send_default_pii);

        let mut sentry_req = sentry_request_from_http(&req, with_pii);
        let name = transaction_name_from_http(&req);

        let transaction = if inner.start_transaction {
            let headers = req.headers().iter().flat_map(|(header, value)| {
                value.to_str().ok().map(|value| (header.as_str(), value))
            });

            let ctx = sentry_core::TransactionContext::continue_from_headers(
                &name,
                "http.server",
                headers,
            );

            let transaction = hub.start_transaction(ctx);
            transaction.set_request(sentry_req.clone());
            Some(transaction)
        } else {
            None
        };

        let svc = self.service.clone();
        async move {
            let mut req = req;

            if should_capture_request_body(req.headers(), max_request_body_size) {
                sentry_req.data = Some(capture_request_body(&mut req).await);
            }

            let parent_span = hub.configure_scope(|scope| {
                let parent_span = scope.get_span();
                if let Some(transaction) = transaction.as_ref() {
                    scope.set_span(Some(transaction.clone().into()));
                } else {
                    scope.set_transaction((!inner.start_transaction).then_some(&name));
                }
                scope.add_event_processor(move |event| Some(process_event(event, &sentry_req)));
                parent_span
            });
            let fut = svc.call(req).bind_hub(hub.clone());
            let mut res: Self::Response = match fut.await {
                Ok(res) => res,
                Err(e) => {
                    if inner.capture_server_errors {
                        hub.capture_error(&e);
                    }

                    if let Some(transaction) = transaction {
                        if transaction.get_status().is_none() {
                            let status = protocol::SpanStatus::UnknownError;
                            transaction.set_status(status);
                        }
                        transaction.finish();
                        hub.configure_scope(|scope| scope.set_span(parent_span));
                    }
                    return Err(e);
                }
            };

            // Response errors
            if inner.capture_server_errors && res.response().status().is_server_error() {
                if let Some(e) = res.response().error() {
                    let event_id = hub.capture_error(e);

                    if inner.emit_header {
                        res.response_mut().headers_mut().insert(
                            "x-sentry-event".parse().unwrap(),
                            event_id.simple().to_string().parse().unwrap(),
                        );
                    }
                }
            }

            if let Some(transaction) = transaction {
                if transaction.get_status().is_none() {
                    let status = map_status(res.status());
                    transaction.set_status(status);
                }
                transaction.finish();
                hub.configure_scope(|scope| scope.set_span(parent_span));
            }

            Ok(res)
        }
        .boxed_local()
    }
}

fn map_status(status: StatusCode) -> protocol::SpanStatus {
    match status {
        StatusCode::UNAUTHORIZED => protocol::SpanStatus::Unauthenticated,
        StatusCode::FORBIDDEN => protocol::SpanStatus::PermissionDenied,
        StatusCode::NOT_FOUND => protocol::SpanStatus::NotFound,
        StatusCode::TOO_MANY_REQUESTS => protocol::SpanStatus::ResourceExhausted,
        status if status.is_client_error() => protocol::SpanStatus::InvalidArgument,
        StatusCode::NOT_IMPLEMENTED => protocol::SpanStatus::Unimplemented,
        StatusCode::SERVICE_UNAVAILABLE => protocol::SpanStatus::Unavailable,
        status if status.is_server_error() => protocol::SpanStatus::InternalError,
        StatusCode::CONFLICT => protocol::SpanStatus::AlreadyExists,
        status if status.is_success() => protocol::SpanStatus::Ok,
        _ => protocol::SpanStatus::UnknownError,
    }
}

/// Extract a transaction name from the HTTP request
fn transaction_name_from_http(req: &ServiceRequest) -> String {
    let path_part = req.match_pattern().unwrap_or_else(|| "<none>".to_string());
    format!("{} {}", req.method(), path_part)
}

/// Build a Sentry request struct from the HTTP request
fn sentry_request_from_http(request: &ServiceRequest, with_pii: bool) -> Request {
    let mut sentry_req = Request {
        url: format!(
            "{}://{}{}",
            request.connection_info().scheme(),
            request.connection_info().host(),
            request.uri()
        )
        .parse()
        .ok(),
        method: Some(request.method().to_string()),
        headers: request
            .headers()
            .iter()
            .filter(|(_, v)| !v.is_sensitive())
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
            .collect(),
        ..Default::default()
    };

    // If PII is enabled, include the remote address
    if with_pii {
        if let Some(remote) = request.connection_info().remote_addr() {
            sentry_req.env.insert("REMOTE_ADDR".into(), remote.into());
        }
    };

    sentry_req
}

/// Add request data to a Sentry event
fn process_event(mut event: Event<'static>, request: &Request) -> Event<'static> {
    // Request
    if event.request.is_none() {
        event.request = Some(request.clone());
    }

    // SDK
    if let Some(sdk) = event.sdk.take() {
        let mut sdk = sdk.into_owned();
        sdk.packages.push(ClientSdkPackage {
            name: "sentry-actix".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        });
        event.sdk = Some(Cow::Owned(sdk));
    }
    event
}

#[cfg(test)]
mod tests {
    use std::io;

    use actix_web::test::{call_service, init_service, TestRequest};
    use actix_web::{get, web, App, HttpRequest, HttpResponse};
    use futures::executor::block_on;

    use sentry::Level;

    use super::*;

    fn _assert_hub_no_events() {
        if Hub::current().last_event_id().is_some() {
            panic!("Current hub should not have had any events.");
        }
    }

    fn _assert_hub_has_events() {
        Hub::current()
            .last_event_id()
            .expect("Current hub should have had events.");
    }

    /// Test explicit events sent to the current Hub inside an Actix service.
    #[actix_web::test]
    async fn test_explicit_events() {
        let events = sentry::test::with_captured_events(|| {
            block_on(async {
                let service = || {
                    // Current Hub should have no events
                    _assert_hub_no_events();

                    sentry::capture_message("Message", Level::Warning);

                    // Current Hub should have the event
                    _assert_hub_has_events();

                    HttpResponse::Ok()
                };

                let app = init_service(
                    App::new()
                        .wrap(Sentry::builder().with_hub(Hub::current()).finish())
                        .service(web::resource("/test").to(service)),
                )
                .await;

                // Call the service twice (sequentially) to ensure the middleware isn't sticky
                for _ in 0..2 {
                    let req = TestRequest::get().uri("/test").to_request();
                    let res = call_service(&app, req).await;
                    assert!(res.status().is_success());
                }
            })
        });

        assert_eq!(events.len(), 2);
        for event in events {
            let request = event.request.expect("Request should be set.");
            assert_eq!(event.transaction, Some("GET /test".into()));
            assert_eq!(event.message, Some("Message".into()));
            assert_eq!(event.level, Level::Warning);
            assert_eq!(request.method, Some("GET".into()));
        }
    }

    /// Test transaction name HTTP verb.
    #[actix_web::test]
    async fn test_match_pattern() {
        let events = sentry::test::with_captured_events(|| {
            block_on(async {
                let service = |_name: String| {
                    // Current Hub should have no events
                    _assert_hub_no_events();

                    sentry::capture_message("Message", Level::Warning);

                    // Current Hub should have the event
                    _assert_hub_has_events();

                    HttpResponse::Ok()
                };

                let app = init_service(
                    App::new()
                        .wrap(Sentry::builder().with_hub(Hub::current()).finish())
                        .service(web::resource("/test/{name}").route(web::post().to(service))),
                )
                .await;

                // Call the service twice (sequentially) to ensure the middleware isn't sticky
                for _ in 0..2 {
                    let req = TestRequest::post().uri("/test/fake_name").to_request();
                    let res = call_service(&app, req).await;
                    assert!(res.status().is_success());
                }
            })
        });

        assert_eq!(events.len(), 2);
        for event in events {
            let request = event.request.expect("Request should be set.");
            assert_eq!(event.transaction, Some("POST /test/{name}".into()));
            assert_eq!(event.message, Some("Message".into()));
            assert_eq!(event.level, Level::Warning);
            assert_eq!(request.method, Some("POST".into()));
        }
    }

    /// Ensures errors returned in the Actix service trigger an event.
    #[actix_web::test]
    async fn test_response_errors() {
        let events = sentry::test::with_captured_events(|| {
            block_on(async {
                #[get("/test")]
                async fn failing(_req: HttpRequest) -> Result<String, Error> {
                    // Current hub should have no events
                    _assert_hub_no_events();

                    Err(io::Error::new(io::ErrorKind::Other, "Test Error").into())
                }

                let app = init_service(
                    App::new()
                        .wrap(Sentry::builder().with_hub(Hub::current()).finish())
                        .service(failing),
                )
                .await;

                // Call the service twice (sequentially) to ensure the middleware isn't sticky
                for _ in 0..2 {
                    let req = TestRequest::get().uri("/test").to_request();
                    let res = call_service(&app, req).await;
                    assert!(res.status().is_server_error());
                }
            })
        });

        assert_eq!(events.len(), 2);
        for event in events {
            let request = event.request.expect("Request should be set.");
            assert_eq!(event.transaction, Some("GET /test".into())); // Transaction name is the matcher of the route
            assert_eq!(event.message, None);
            assert_eq!(event.exception.values[0].ty, String::from("Custom"));
            assert_eq!(event.exception.values[0].value, Some("Test Error".into()));
            assert_eq!(event.level, Level::Error);
            assert_eq!(request.method, Some("GET".into()));
        }
    }

    /// Ensures client errors (4xx) are not captured.
    #[actix_web::test]
    async fn test_client_errors_discarded() {
        let events = sentry::test::with_captured_events(|| {
            block_on(async {
                let service = HttpResponse::NotFound;

                let app = init_service(
                    App::new()
                        .wrap(Sentry::builder().with_hub(Hub::current()).finish())
                        .service(web::resource("/test").to(service)),
                )
                .await;

                let req = TestRequest::get().uri("/test").to_request();
                let res = call_service(&app, req).await;
                assert!(res.status().is_client_error());
            })
        });

        assert!(events.is_empty());
    }

    /// Ensures transaction name can be overridden in handler scope.
    #[actix_web::test]
    async fn test_override_transaction_name() {
        let events = sentry::test::with_captured_events(|| {
            block_on(async {
                #[get("/test")]
                async fn original_transaction(_req: HttpRequest) -> Result<String, Error> {
                    // Override transaction name
                    sentry::configure_scope(|scope| scope.set_transaction(Some("new_transaction")));
                    Err(io::Error::new(io::ErrorKind::Other, "Test Error").into())
                }

                let app = init_service(
                    App::new()
                        .wrap(Sentry::builder().with_hub(Hub::current()).finish())
                        .service(original_transaction),
                )
                .await;

                let req = TestRequest::get().uri("/test").to_request();
                let res = call_service(&app, req).await;
                assert!(res.status().is_server_error());
            })
        });

        assert_eq!(events.len(), 1);
        let event = events[0].clone();
        let request = event.request.expect("Request should be set.");
        assert_eq!(event.transaction, Some("new_transaction".into())); // Transaction name is overridden by handler
        assert_eq!(event.message, None);
        assert_eq!(event.exception.values[0].ty, String::from("Custom"));
        assert_eq!(event.exception.values[0].value, Some("Test Error".into()));
        assert_eq!(event.level, Level::Error);
        assert_eq!(request.method, Some("GET".into()));
    }

    #[actix_web::test]
    async fn test_track_session() {
        let envelopes = sentry::test::with_captured_envelopes_options(
            || {
                block_on(async {
                    #[get("/")]
                    async fn hello() -> impl actix_web::Responder {
                        String::from("Hello there!")
                    }

                    let middleware = Sentry::builder().with_hub(Hub::current()).finish();

                    let app = init_service(App::new().wrap(middleware).service(hello)).await;

                    for _ in 0..5 {
                        let req = TestRequest::get().uri("/").to_request();
                        call_service(&app, req).await;
                    }
                })
            },
            sentry::ClientOptions {
                release: Some("some-release".into()),
                session_mode: sentry::SessionMode::Request,
                auto_session_tracking: true,
                ..Default::default()
            },
        );
        assert_eq!(envelopes.len(), 1);

        let mut items = envelopes[0].items();
        if let Some(sentry::protocol::EnvelopeItem::SessionAggregates(aggregate)) = items.next() {
            let aggregates = &aggregate.aggregates;

            assert_eq!(aggregates[0].distinct_id, None);
            assert_eq!(aggregates[0].exited, 5);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }
}
