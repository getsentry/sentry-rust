#![expect(missing_docs, reason = "predates lint enforcement")]
#![cfg(feature = "logs")]

use sentry::protocol::{EnvelopeItem, ItemContainer, LogLevel};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;

#[tracing::instrument(fields(tags.tag = "key", not_tag = "value"))]
fn function_with_tags(value: i32) {
    tracing::error!(value, "event");
}

#[test]
fn should_instrument_function_with_log() {
    let _subscriber = tracing_subscriber::registry()
        .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
        .with(sentry_tracing::log_layer().with_filter(LevelFilter::INFO))
        .set_default();

    let envelopes = sentry::test::with_captured_envelopes_options(
        || function_with_tags(1),
        sentry::ClientOptions::new().traces_sample_rate(1.0),
    );
    let mut transaction = None;
    let mut logs = Vec::new();
    for item in envelopes.iter().flat_map(|envelope| envelope.items()) {
        match item {
            EnvelopeItem::Transaction(item) => assert!(transaction.replace(item).is_none()),
            EnvelopeItem::ItemContainer(ItemContainer::Logs(items)) => logs.extend(items.iter()),
            _ => (),
        }
    }

    let transaction = transaction.expect("transaction");
    assert_eq!(transaction.name.as_deref(), Some("function_with_tags"));
    assert_eq!(transaction.tags.len(), 1);
    assert_eq!(transaction.tags.get("tag").map(String::as_str), Some("key"));

    let trace = match transaction.contexts.get("trace").expect("trace context") {
        sentry::protocol::Context::Trace(trace) => trace,
        unexpected => panic!("Expected trace context, but got {unexpected:?}"),
    };
    assert_eq!(trace.op.as_deref(), Some("smoke::function_with_tags"));
    assert_eq!(trace.data.get("not_tag"), Some(&"value".into()));
    assert_eq!(trace.data.get("value"), Some(&1.into()));
    assert_eq!(trace.data.len(), 6);
    assert_eq!(
        trace.data.get("sentry.tracing.target"),
        Some("smoke".into()).as_ref()
    );
    assert_eq!(
        trace.data.get("code.module.name"),
        Some("smoke".into()).as_ref()
    );
    assert!(trace.data.contains_key("code.file.path"));
    assert!(trace.data.contains_key("code.line.number"));

    assert_eq!(logs.len(), 1);
    let log = logs[0];
    assert_eq!(log.level, LogLevel::Error);
    assert_eq!(log.body, "event");
    assert_eq!(log.trace_id.as_ref(), Some(&trace.trace_id));
    assert_eq!(log.attributes.get("value"), Some(&1.into()));
}
