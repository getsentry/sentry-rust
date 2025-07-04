use std::convert::TryInto;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use http::{header, uri, Request, Response, StatusCode};
use pin_project::pinned_drop;
use sentry_core::utils::is_sensitive_header;
use sentry_core::{protocol, Hub};
use tower_layer::Layer;
use tower_service::Service;

/// Tower Layer that captures Http Request information.
///
/// The Service created by this Layer can optionally start a new
/// performance monitoring transaction for each incoming request,
/// continuing the trace based on incoming distributed tracing headers.
///
/// The created transaction will automatically use the request URI as its name.
/// This is sometimes not desirable in case the request URI contains unique IDs
/// or similar. In this case, users should manually override the transaction name
/// in the request handler using the [`Scope::set_transaction`](sentry_core::Scope::set_transaction)
/// method.
///
/// By default, the service will filter out potentially sensitive headers from the captured
/// requests. By enabling `with_pii`, you can opt in to capturing all headers instead.
#[derive(Clone, Default)]
pub struct SentryHttpLayer {
    start_transaction: bool,
    with_pii: bool,
}

impl SentryHttpLayer {
    /// Creates a new Layer that only captures request information.
    /// If a client is bound to the main Hub (i.e. the SDK has already been initialized), set `with_pii` based on the `send_default_pii` client option.
    pub fn new() -> Self {
        let mut slf = Self::default();
        Hub::main()
            .client()
            .inspect(|client| slf.with_pii = client.options().send_default_pii);
        slf
    }

    /// Creates a new Layer which starts a new performance monitoring transaction
    /// for each incoming request.
    #[deprecated(since = "0.38.0", note = "please use `enable_transaction` instead")]
    pub fn with_transaction() -> Self {
        Self {
            start_transaction: true,
            with_pii: false,
        }
    }

    /// Enable starting a new performance monitoring transaction for each incoming request.
    #[must_use]
    pub fn enable_transaction(mut self) -> Self {
        self.start_transaction = true;
        self
    }

    /// Include PII in captured requests. Potentially sensitive headers are not filtered out.
    #[must_use]
    pub fn enable_pii(mut self) -> Self {
        self.with_pii = true;
        self
    }
}

/// Tower Service that captures Http Request information.
///
/// The Service can optionally start a new performance monitoring transaction
/// for each incoming request, continuing the trace based on incoming
/// distributed tracing headers.
///
/// If `with_pii` is disabled, sensitive headers will be filtered out.
#[derive(Clone)]
pub struct SentryHttpService<S> {
    service: S,
    start_transaction: bool,
    with_pii: bool,
}

impl<S> Layer<S> for SentryHttpLayer {
    type Service = SentryHttpService<S>;

    fn layer(&self, service: S) -> Self::Service {
        Self::Service {
            service,
            start_transaction: self.start_transaction,
            with_pii: self.with_pii,
        }
    }
}

/// The Future returned from [`SentryHttpService`].
#[pin_project::pin_project(PinnedDrop)]
pub struct SentryHttpFuture<F> {
    on_first_poll: Option<(
        sentry_core::protocol::Request,
        Option<sentry_core::TransactionContext>,
    )>,
    transaction: Option<(
        sentry_core::TransactionOrSpan,
        Option<sentry_core::TransactionOrSpan>,
    )>,
    #[pin]
    future: F,
}

impl<F, ResBody, Error> Future for SentryHttpFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, Error>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.project();
        if let Some((sentry_req, trx_ctx)) = slf.on_first_poll.take() {
            sentry_core::configure_scope(|scope| {
                if let Some(trx_ctx) = trx_ctx {
                    let transaction: sentry_core::TransactionOrSpan =
                        sentry_core::start_transaction(trx_ctx).into();
                    transaction.set_request(sentry_req.clone());
                    transaction.set_data("origin", "auto.http.tower".into());
                    let parent_span = scope.get_span();
                    scope.set_span(Some(transaction.clone()));
                    *slf.transaction = Some((transaction, parent_span));
                }

                scope.add_event_processor(move |mut event| {
                    if event.request.is_none() {
                        event.request = Some(sentry_req.clone());
                    }
                    Some(event)
                });
            });
        }
        match slf.future.poll(cx) {
            Poll::Ready(res) => {
                if let Some((transaction, parent_span)) = slf.transaction.take() {
                    if transaction.get_status().is_none() {
                        let status = match &res {
                            Ok(res) => map_status(res.status()),
                            Err(_) => protocol::SpanStatus::UnknownError,
                        };
                        transaction.set_status(status);
                    }
                    transaction.finish();
                    sentry_core::configure_scope(|scope| scope.set_span(parent_span));
                }
                Poll::Ready(res)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pinned_drop]
impl<F> PinnedDrop for SentryHttpFuture<F> {
    fn drop(self: Pin<&mut Self>) {
        let slf = self.project();

        // If the future gets dropped without being polled to completion,
        // still finish the transaction to make sure this is not lost.
        if let Some((transaction, parent_span)) = slf.transaction.take() {
            if transaction.get_status().is_none() {
                transaction.set_status(protocol::SpanStatus::Aborted);
            }
            transaction.finish();
            sentry_core::configure_scope(|scope| scope.set_span(parent_span));
        }
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for SentryHttpService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = SentryHttpFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        let sentry_req = sentry_core::protocol::Request {
            method: Some(request.method().to_string()),
            url: get_url_from_request(&request),
            headers: request
                .headers()
                .into_iter()
                .filter(|(_, value)| !value.is_sensitive())
                .filter(|(header, _)| self.with_pii || !is_sensitive_header(header.as_str()))
                .map(|(header, value)| {
                    (
                        header.to_string(),
                        value.to_str().unwrap_or_default().into(),
                    )
                })
                .collect(),
            ..Default::default()
        };
        let trx_ctx = if self.start_transaction {
            let headers = request.headers().into_iter().flat_map(|(header, value)| {
                value.to_str().ok().map(|value| (header.as_str(), value))
            });
            let tx_name = format!("{} {}", request.method(), path_from_request(&request));
            Some(sentry_core::TransactionContext::continue_from_headers(
                &tx_name,
                "http.server",
                headers,
            ))
        } else {
            None
        };

        SentryHttpFuture {
            on_first_poll: Some((sentry_req, trx_ctx)),
            transaction: None,
            future: self.service.call(request),
        }
    }
}

fn path_from_request<B>(request: &Request<B>) -> &str {
    #[cfg(feature = "axum-matched-path")]
    if let Some(matched_path) = request.extensions().get::<axum::extract::MatchedPath>() {
        return matched_path.as_str();
    }

    request.uri().path()
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

fn get_url_from_request<B>(request: &Request<B>) -> Option<url::Url> {
    let uri = request.uri().clone();
    let mut uri_parts = uri.into_parts();
    uri_parts.scheme.get_or_insert(uri::Scheme::HTTP);
    if uri_parts.authority.is_none() {
        let host = request.headers().get(header::HOST)?.as_bytes();
        uri_parts.authority = Some(host.try_into().ok()?);
    }
    let uri = uri::Uri::from_parts(uri_parts).ok()?;
    uri.to_string().parse().ok()
}
