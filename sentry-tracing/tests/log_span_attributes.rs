//! Tests that `LogLayer::enable_span_attributes` attaches parent span attributes to captured logs.

#![cfg(feature = "logs")]

use sentry::protocol::{EnvelopeItem, ItemContainer, LogLevel};
use sentry_tracing::{LogLayer, SpanLayer};
use tracing::Level;
use tracing_subscriber::filter::{filter_fn, LevelFilter};
use tracing_subscriber::prelude::*;

#[test]
fn log_layer_captures_span_attributes() {
    let _subscriber = tracing_subscriber::registry()
        .with(SpanLayer::new().with_filter(LevelFilter::INFO))
        .with(
            LogLayer::new()
                .enable_span_attributes()
                .with_filter(filter_fn(|metadata| {
                    metadata.is_span() || *metadata.level() == Level::INFO
                })),
        )
        .set_default();

    let envelopes = sentry::test::with_captured_envelopes_options(
        foo,
        sentry::ClientOptions::new().traces_sample_rate(0.0),
    );
    let mut logs = Vec::new();
    for item in envelopes.iter().flat_map(|envelope| envelope.items()) {
        if let EnvelopeItem::ItemContainer(ItemContainer::Logs(items)) = item {
            logs.extend(items.iter());
        }
    }

    assert_eq!(logs.len(), 1);
    let log = logs[0];
    assert_eq!(log.level, LogLevel::Info);
    assert_eq!(log.body, "executing foo");
    assert_eq!(
        log.attributes.get("foo:contextual_value").cloned(),
        Some(42.into())
    );
}

#[tracing::instrument(fields(contextual_value = 42))]
fn foo() {
    tracing::info!("executing foo");
}
