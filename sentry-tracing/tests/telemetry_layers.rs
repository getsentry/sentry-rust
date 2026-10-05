#![expect(missing_docs, reason = "integration tests")]

use sentry::protocol::EnvelopeItem;
#[cfg(feature = "logs")]
use sentry::protocol::{Context, ItemContainer, LogLevel, Value};
use tracing_subscriber::filter::{filter_fn, LevelFilter};
use tracing_subscriber::prelude::*;

#[test]
fn span_layer_does_not_capture_event_telemetry() {
    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_tracing::span_layer())
        .set_default();

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let _span = tracing::info_span!("operation").entered();
            tracing::info!("info");
            tracing::error!("error");
        },
        sentry::ClientOptions::new().traces_sample_rate(1.0),
    );

    assert_eq!(envelopes.len(), 1);
    assert!(matches!(
        envelopes[0].items().next(),
        Some(EnvelopeItem::Transaction(_))
    ));
    assert_eq!(envelopes[0].items().count(), 1);
}

#[cfg(feature = "logs")]
#[test]
fn spans_and_logs_do_not_capture_errors_or_breadcrumbs() {
    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
        .with(sentry_tracing::log_layer().with_filter(LevelFilter::INFO))
        .set_default();

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let _span = tracing::info_span!("operation").entered();
            tracing::debug!("ignored");
            tracing::info!("info");
            tracing::error!("error");
        },
        sentry::ClientOptions::new().traces_sample_rate(1.0),
    );

    let mut transactions = 0;
    let mut logs = Vec::new();
    for item in envelopes.iter().flat_map(|envelope| envelope.items()) {
        match item {
            EnvelopeItem::Transaction(_) => transactions += 1,
            EnvelopeItem::ItemContainer(ItemContainer::Logs(items)) => logs.extend(items),
            other => panic!("unexpected item: {other:?}"),
        }
    }
    assert_eq!(transactions, 1);
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].body, "info");
    assert_eq!(logs[1].body, "error");
}

#[cfg(feature = "logs")]
#[test]
fn all_layers_capture_their_default_levels_and_parent_fields() {
    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
        .with(
            sentry_tracing::breadcrumb_layer()
                .enable_span_attributes()
                // `WARN` and `INFO` only; `ERROR` is captured by the error layer.
                .with_filter(filter_fn(|m| {
                    matches!(*m.level(), tracing::Level::WARN | tracing::Level::INFO)
                })),
        )
        .with(
            sentry_tracing::error_layer()
                .enable_span_attributes()
                .with_filter(filter_fn(|m| {
                    m.is_span() || *m.level() == tracing::Level::ERROR
                })),
        )
        .with(
            sentry_tracing::log_layer()
                .enable_span_attributes()
                .with_filter(LevelFilter::INFO),
        )
        .set_default();

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let _span = tracing::info_span!("operation", contextual_value = 42).entered();
            tracing::debug!("ignored");
            tracing::info!("info");
            tracing::warn!("warning");
            tracing::error!("error");
        },
        sentry::ClientOptions::new().traces_sample_rate(1.0),
    );

    let mut transactions = 0;
    let mut events = Vec::new();
    let mut logs = Vec::new();
    for item in envelopes.iter().flat_map(|envelope| envelope.items()) {
        match item {
            EnvelopeItem::Transaction(_) => transactions += 1,
            EnvelopeItem::Event(event) => events.push(event),
            EnvelopeItem::ItemContainer(ItemContainer::Logs(items)) => logs.extend(items),
            other => panic!("unexpected item: {other:?}"),
        }
    }
    assert_eq!(transactions, 1);
    assert_eq!(logs.len(), 3);
    assert_eq!(
        logs.iter().map(|log| log.level).collect::<Vec<_>>(),
        [LogLevel::Info, LogLevel::Warn, LogLevel::Error,]
    );
    assert_eq!(
        logs[0]
            .attributes
            .get("operation:contextual_value")
            .cloned(),
        Some(42.into())
    );

    assert_eq!(events.len(), 1);
    let event = events[0];
    assert_eq!(event.message.as_deref(), Some("error"));
    assert_eq!(event.breadcrumbs.len(), 2);
    assert_eq!(event.breadcrumbs[0].message.as_deref(), Some("info"));
    assert_eq!(event.breadcrumbs[1].message.as_deref(), Some("warning"));
    assert_eq!(
        event.breadcrumbs[0].data.get("operation:contextual_value"),
        Some(&Value::from(42))
    );
    match event.contexts.get("Rust Tracing Fields") {
        Some(Context::Other(fields)) => assert_eq!(
            fields.get("operation:contextual_value"),
            Some(&Value::from(42))
        ),
        other => panic!("missing span fields: {other:?}"),
    }
}
