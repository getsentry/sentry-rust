//! Tests standalone error/breadcrumb capture and span context without transaction sampling.

use sentry::protocol::{Context, EnvelopeItem};
use tracing::Level;
use tracing_subscriber::filter::{filter_fn, LevelFilter};
use tracing_subscriber::prelude::*;

#[test]
fn error_and_breadcrumb_layers_capture_unsampled_span_context() {
    let _subscriber = tracing_subscriber::registry()
        .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
        .with(
            sentry_tracing::breadcrumb_layer()
                .enable_span_attributes()
                .with_filter(filter_fn(|metadata| {
                    metadata.is_span() || matches!(*metadata.level(), Level::INFO | Level::WARN)
                })),
        )
        .with(
            sentry_tracing::error_layer()
                .enable_span_attributes()
                .with_filter(filter_fn(|metadata| {
                    metadata.is_span() || *metadata.level() == Level::ERROR
                })),
        )
        .set_default();

    let mut expected_trace = None;
    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let _span = tracing::info_span!("request", order_id = 42).entered();
            expected_trace = sentry::configure_scope(|scope| {
                scope.get_span().map(|span| span.get_trace_context())
            });
            tracing::info!("processing order");
            tracing::error!("order failed");
        },
        sentry::ClientOptions::new().traces_sample_rate(0.0),
    );

    let mut errors = Vec::new();
    for item in envelopes.iter().flat_map(|envelope| envelope.items()) {
        match item {
            EnvelopeItem::Event(error) => errors.push(error),
            unexpected => panic!("expected only an error event, got {unexpected:?}"),
        }
    }
    assert_eq!(errors.len(), 1);
    let error = errors[0];
    assert_eq!(error.level, sentry::Level::Error);
    assert_eq!(error.message.as_deref(), Some("order failed"));

    assert_eq!(error.breadcrumbs.len(), 1);
    let breadcrumb = &error.breadcrumbs[0];
    assert_eq!(breadcrumb.level, sentry::Level::Info);
    assert_eq!(breadcrumb.message.as_deref(), Some("processing order"));
    assert_eq!(breadcrumb.data.get("request:order_id"), Some(&42.into()));

    match error.contexts.get("Rust Tracing Fields") {
        Some(Context::Other(fields)) => {
            assert_eq!(fields.get("request:order_id"), Some(&42.into()));
        }
        unexpected => panic!("expected tracing fields, got {unexpected:?}"),
    }

    let expected_trace = expected_trace.expect("expected an active Sentry span");
    match error.contexts.get("trace") {
        Some(Context::Trace(trace)) => {
            assert_eq!(trace.trace_id, expected_trace.trace_id);
            assert_eq!(trace.span_id, expected_trace.span_id);
            assert_eq!(trace.op, expected_trace.op);
        }
        unexpected => panic!("expected trace context, got {unexpected:?}"),
    }
}
