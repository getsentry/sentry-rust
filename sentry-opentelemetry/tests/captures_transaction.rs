mod shared;

use opentelemetry::{
    global,
    trace::{TraceContextExt, Tracer, TracerProvider},
    Context,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use sentry_opentelemetry::{SentryPropagator, SentrySpanProcessor};

#[test]
fn test_captures_transaction() {
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

    // End the span
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
        }
        unexpected => panic!("Expected transaction, but got {:#?}", unexpected),
    }
}
