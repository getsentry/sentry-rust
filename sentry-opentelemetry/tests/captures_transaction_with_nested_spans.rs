mod shared;

use opentelemetry::{
    global,
    propagation::TextMapPropagator,
    trace::{Status, TraceContextExt, Tracer, TracerProvider},
    Context, KeyValue,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use sentry_core::protocol::Transaction;
use sentry_opentelemetry::{SentryPropagator, SentrySpanProcessor};
use std::collections::HashMap;

#[test]
fn test_captures_transaction_with_nested_spans() {
    // Initialize Sentry
    let transport = shared::init_sentry(1.0); // Sample all spans

    // Set up OpenTelemetry
    global::set_text_map_propagator(SentryPropagator::new());
    let tracer_provider = SdkTracerProvider::builder()
        .with_span_processor(SentrySpanProcessor::new())
        .build();
    let tracer = tracer_provider.tracer("test".to_string());

    // Create root span
    let root_span = tracer.start("root_span");
    let cx = Context::current_with_span(root_span);

    // Create child span
    let child_span = tracer.start_with_context("child_span", &cx);
    let child_cx = cx.with_span(child_span);

    // Create another child span
    let grandchild_span = tracer.start_with_context("grandchild_span", &child_cx);
    let grandchild_cx = child_cx.with_span(grandchild_span);

    // Add some attributes to the grandchild
    grandchild_cx
        .span()
        .set_attribute(KeyValue::new("test.key", "test.value"));
    grandchild_cx.span().set_status(Status::Ok);

    // End spans in reverse order
    grandchild_cx.span().end();
    child_cx.span().end();
    cx.span().end();

    // Check that data was sent to Sentry
    let envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(
        envelopes.len(),
        1,
        "Expected one transaction to be sent to Sentry"
    );

    let transaction = envelopes[0].items().next().unwrap();
    match transaction {
        sentry::protocol::EnvelopeItem::Transaction(tx) => {
            assert_eq!(tx.name.as_deref(), Some("root_span"));
            assert_eq!(tx.spans.len(), 2); // Should have 2 child spans

            let child_span = tx
                .spans
                .iter()
                .find(|s| s.description.as_deref() == Some("child_span"))
                .expect("Child span should exist");
            let grandchild_span = tx
                .spans
                .iter()
                .find(|s| s.description.as_deref() == Some("grandchild_span"))
                .expect("Grandchild span should exist");

            // Get transaction span ID from trace context
            let tx_span_id = match &tx.contexts.get("trace") {
                Some(sentry::protocol::Context::Trace(trace)) => trace.span_id,
                _ => panic!("Missing trace context in transaction"),
            };

            // Check parent-child relationship
            assert_eq!(grandchild_span.parent_span_id, Some(child_span.span_id));
            assert_eq!(child_span.parent_span_id, Some(tx_span_id));
        }
        unexpected => panic!("Expected transaction, but got {:#?}", unexpected),
    }
}
