use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tower_layer::Layer;
use tower_service::Service;

#[derive(Clone)]
pub struct SentryHttpLayer;

#[derive(Clone)]
pub struct SentryHttpService<S> {
    service: S,
}

impl<S> Layer<S> for SentryHttpLayer {
    type Service = SentryHttpService<S>;

    fn layer(&self, service: S) -> Self::Service {
        Self::Service { service }
    }
}

pub struct SentryHttpFuture<F> {
    transaction: Option<sentry_core::Transaction>,
    future: F,
}

impl Future for SentryHttpFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field
        let future = unsafe { self.map_unchecked_mut(|s| &mut s.future) };
        match future.poll(cx) {
            Poll::Ready(res) => {
                if let Some(transaction) = self.transaction.take() {
                    transaction.finish();
                }
                Poll::Ready(res)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S, Body> Service<Request<Body>> for SentryHttpService<S>
where
    S: Service<Request<Body>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = SentryHttpFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let transaction = sentry::configure_scope(|scope| {
            // https://develop.sentry.dev/sdk/event-payloads/request/
            let mut ctx = BTreeMap::new();

            // TODO: method, url, query_string

            // headers
            let mut headers = BTreeMap::new();
            for (header, value) in request.headers() {
                headers.insert(
                    header.to_string(),
                    value.to_str().unwrap_or("<Opaque header value>").into(),
                );
            }
            ctx.insert("headers".into(), headers);

            scope.set_context("request", sentry_core::protocol::Context::Other(ctx));

            // TODO: maybe make transaction creation optional?
            let transaction = if true {
                let tx_ctx = sentry_core::TransactionContext::continue_from_headers(
                    // TODO: whats the name here?
                    "",
                    "http",
                    request.headers(),
                );
                Some(sentry_core::start_transaction(tx_ctx))
            } else {
                None
            };

            scope.set_span(transaction.clone());
            transaction
        });

        SentryHttpFuture {
            transaction,
            future: self.service.call(request),
        }
    }
}
