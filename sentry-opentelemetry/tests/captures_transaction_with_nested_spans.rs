mod shared;

use opentelemetry::{
    global,
    trace::{Status, TraceContextExt, Tracer, TracerProvider},
    KeyValue,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use sentry_opentelemetry::{SentryPropagator, SentrySpanProcessor};

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

    // Create nested spans using in_span
    tracer.in_span("root_span", |_| {
        tracer.in_span("child_span", |_| {
            tracer.in_span("grandchild_span", |cx| {
                // Add some attributes to the grandchild
                cx.span()
                    .set_attribute(KeyValue::new("test.key", "test.value"));
                cx.span().set_status(Status::Ok);
            });
        });
    });

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
