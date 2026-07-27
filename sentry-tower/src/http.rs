use std::convert::TryInto;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};

use http::{header, uri, Request, Response, StatusCode};
use pin_project::pinned_drop;
use sentry_core::utils::{is_sensitive_header, scrub_pii_from_url};
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
                    let transaction = sentry_core::start_transaction(trx_ctx);
                    transaction.set_origin("auto.http.tower");
                    let transaction: sentry_core::TransactionOrSpan = transaction.into();
                    transaction.set_request(sentry_req.clone());
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
        let sentry_req = sentry_request_from_http(&request, self.with_pii);
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

fn sentry_request_from_http<B>(request: &Request<B>, with_pii: bool) -> protocol::Request {
    let mut sentry_req = protocol::Request {
        method: Some(request.method().to_string()),
        url: get_url_from_request(request).map(scrub_pii_from_url),
        headers: request
            .headers()
            .into_iter()
            .filter(|(_, value)| !value.is_sensitive())
            .filter(|(header, _)| with_pii || !is_sensitive_header(header.as_str()))
            .map(|(header, value)| {
                (
                    header.to_string(),
                    value.to_str().unwrap_or_default().into(),
                )
            })
            .collect(),
        ..Default::default()
    };

    if with_pii {
        if let Some(remote_addr) = remote_addr_from_request(request) {
            sentry_req.env.insert("REMOTE_ADDR".into(), remote_addr);
        }
    }

    sentry_req
}

fn remote_addr_from_request<B>(request: &Request<B>) -> Option<String> {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .map(ToOwned::to_owned)
        .or_else(|| {
            request
                .extensions()
                .get::<SocketAddr>()
                .map(|address| address.ip().to_string())
        })
        .or_else(|| {
            request
                .extensions()
                .get::<IpAddr>()
                .map(ToString::to_string)
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_first_forwarded_address_with_pii() {
        let request = Request::builder()
            .header("host", "example.com")
            .header("x-forwarded-for", " 203.0.113.9, 10.0.0.1")
            .body(())
            .unwrap();

        let sentry_req = sentry_request_from_http(&request, true);

        assert_eq!(
            sentry_req.env.get("REMOTE_ADDR").map(String::as_str),
            Some("203.0.113.9")
        );
    }

    #[test]
    fn captures_real_ip_when_forwarded_address_is_empty() {
        let request = Request::builder()
            .header("host", "example.com")
            .header("x-forwarded-for", " ")
            .header("x-real-ip", "198.51.100.7")
            .body(())
            .unwrap();

        let sentry_req = sentry_request_from_http(&request, true);

        assert_eq!(
            sentry_req.env.get("REMOTE_ADDR").map(String::as_str),
            Some("198.51.100.7")
        );
    }

    #[test]
    fn captures_socket_address_when_proxy_headers_are_absent() {
        let mut request = Request::builder()
            .header("host", "example.com")
            .body(())
            .unwrap();
        request
            .extensions_mut()
            .insert("192.0.2.4:8080".parse::<SocketAddr>().unwrap());

        let sentry_req = sentry_request_from_http(&request, true);

        assert_eq!(
            sentry_req.env.get("REMOTE_ADDR").map(String::as_str),
            Some("192.0.2.4")
        );
    }

    #[test]
    fn omits_remote_address_without_pii() {
        let request = Request::builder()
            .header("host", "example.com")
            .header("x-forwarded-for", "203.0.113.9")
            .body(())
            .unwrap();

        let sentry_req = sentry_request_from_http(&request, false);

        assert!(!sentry_req.env.contains_key("REMOTE_ADDR"));
        assert!(!sentry_req.headers.contains_key("x-forwarded-for"));
    }
}
