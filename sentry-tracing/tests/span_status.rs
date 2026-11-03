//! Tests that the `sentry.status` span field sets the Sentry span status.

mod shared;

use sentry::protocol::{Context, EnvelopeItem, SpanStatus, TraceContext, Transaction};
use sentry_tracing::SpanLayer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;

#[tracing::instrument(fields(sentry.status = "internal_error"))]
fn status_on_creation() {}

#[tracing::instrument(fields(sentry.status = tracing::field::Empty))]
fn status_recorded_later() {
    tracing::Span::current().record("sentry.status", "internal_error");
}

#[tracing::instrument(fields(sentry.status = "not a status"))]
fn invalid_status() {}

#[tracing::instrument]
fn parent_with_failing_child() {
    failing_child();
}

#[tracing::instrument(fields(sentry.status = tracing::field::Empty))]
fn failing_child() {
    tracing::Span::current().record("sentry.status", "not_found");
}

fn capture_transaction(f: fn()) -> Transaction<'static> {
    let transport = shared::init_sentry(1.0);
    let _subscriber = tracing_subscriber::registry()
        .with(SpanLayer::new().with_filter(LevelFilter::INFO))
        .set_default();

    f();

    let envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(envelopes.len(), 1);
    match envelopes[0].items().next().unwrap() {
        EnvelopeItem::Transaction(transaction) => *transaction.clone(),
        unexpected => panic!("Expected transaction, but got {unexpected:#?}"),
    }
}

fn trace_context<'a>(transaction: &'a Transaction<'_>) -> &'a TraceContext {
    match transaction.contexts.get("trace") {
        Some(Context::Trace(trace)) => trace,
        _ => panic!("expected trace context"),
    }
}

#[test]
fn sets_status_from_field() {
    for f in [status_on_creation, status_recorded_later] {
        let transaction = capture_transaction(f);
        let trace = trace_context(&transaction);
        assert_eq!(trace.status, Some(SpanStatus::InternalError));
        assert!(!trace.data.contains_key("sentry.status"));
    }
}

#[test]
fn ignores_invalid_status() {
    let transaction = capture_transaction(invalid_status);
    let trace = trace_context(&transaction);
    assert_eq!(trace.status, None);
    assert!(!trace.data.contains_key("sentry.status"));
}

#[test]
fn sets_status_on_child_span() {
    let transaction = capture_transaction(parent_with_failing_child);
    assert_eq!(trace_context(&transaction).status, None);
    assert_eq!(transaction.spans.len(), 1);
    let span = &transaction.spans[0];
    assert_eq!(span.status, Some(SpanStatus::NotFound));
    assert!(!span.data.contains_key("sentry.status"));
}
