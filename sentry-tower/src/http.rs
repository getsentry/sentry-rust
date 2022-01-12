use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use http_::{Request, Response, StatusCode};
use sentry_core::protocol;
use tower_layer::Layer;
use tower_service::Service;

/// Tower Layer that logs Http Request Headers.
///
/// The Service created by this Layer can also optionally start a new
/// performance monitoring transaction for each incoming request,
/// continuing the trace based on incoming distributed tracing headers.
#[derive(Clone, Default)]
pub struct SentryHttpLayer {
    start_transaction: bool,
}

impl SentryHttpLayer {
    /// Creates a new Layer that only logs Request Headers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new Layer which starts a new performance monitoring transaction
    /// for each incoming request.
    pub fn with_transaction() -> Self {
        Self {
            start_transaction: true,
        }
    }
}

/// Tower Service that logs Http Request Headers.
///
/// The Service can also optionally start a new performance monitoring transaction
/// for each incoming request, continuing the trace based on incoming
/// distributed tracing headers.
#[derive(Clone)]
pub struct SentryHttpService<S> {
    service: S,
    start_transaction: bool,
}

impl<S> Layer<S> for SentryHttpLayer {
    type Service = SentryHttpService<S>;

    fn layer(&self, service: S) -> Self::Service {
        Self::Service {
            service,
            start_transaction: self.start_transaction,
        }
    }
}

/// The Future returned from [`SentryHttpService`].
#[pin_project::pin_project]
pub struct SentryHttpFuture<F> {
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
        let transaction = sentry_core::configure_scope(|scope| {
            let sentry_req = sentry_core::protocol::Request {
                method: Some(request.method().to_string()),
                url: request.uri().to_string().parse().ok(),
                headers: request
                    .headers()
                    .into_iter()
                    .map(|(header, value)| {
                        (
                            header.to_string(),
                            value.to_str().unwrap_or_default().into(),
                        )
                    })
                    .collect(),
                ..Default::default()
            };
            scope.add_event_processor(move |mut event| {
                if event.request.is_none() {
                    event.request = Some(sentry_req.clone());
                }
                Some(event)
            });

            if self.start_transaction {
                let headers = request.headers().into_iter().flat_map(|(header, value)| {
                    value.to_str().ok().map(|value| (header.as_str(), value))
                });
                let tx_name = format!("{} {}", request.method(), request.uri().path());
                let tx_ctx = sentry_core::TransactionContext::continue_from_headers(
                    &tx_name,
                    "http.server",
                    headers,
                );
                let transaction: sentry_core::TransactionOrSpan =
                    sentry_core::start_transaction(tx_ctx).into();
                let parent_span = scope.get_span();
                scope.set_span(Some(transaction.clone()));
                Some((transaction, parent_span))
            } else {
                None
            }
        });

        SentryHttpFuture {
            transaction,
            future: self.service.call(request),
        }
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
