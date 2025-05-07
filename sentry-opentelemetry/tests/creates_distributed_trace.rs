mod shared;

use opentelemetry::{
    global,
    propagation::TextMapPropagator,
    trace::{TraceContextExt, Tracer, TracerProvider},
    Context,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use sentry_opentelemetry::{SentryPropagator, SentrySpanProcessor};
use std::collections::HashMap;

#[test]
fn test_creates_distributed_trace() {
    let transport = shared::init_sentry(1.0); // Sample all spans

    // Set up OpenTelemetry
    global::set_text_map_propagator(SentryPropagator::new());
    let tracer_provider = SdkTracerProvider::builder()
        .with_span_processor(SentrySpanProcessor::new())
        .build();
    let tracer = tracer_provider.tracer("test".to_string());

    // We need to store the context to pass between services, so we'll use a mutable variable
    let mut headers = HashMap::new();
    let propagator = SentryPropagator::new();

    // Create a "first service" span and store context in headers
    tracer.in_span("first_service", |first_service_ctx| {
        // Simulate passing the context to another service by extracting and injecting e.g. HTTP headers
        propagator.inject_context(&first_service_ctx, &mut TestInjector(&mut headers));
    });

    // Now simulate the second service receiving the headers and continuing the trace
    let second_service_ctx =
        propagator.extract_with_context(&Context::current(), &TestExtractor(&headers));

    // Create a second service span that continues the trace
    // We need to use start_with_context here to connect with the previous context
    let second_service_span = tracer.start_with_context("second_service", &second_service_ctx);
    let second_service_ctx = second_service_ctx.with_span(second_service_span);

    // End the second service span
    second_service_ctx.span().end();

    // Get both transactions at once
    let envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(
        envelopes.len(),
        2,
        "Expected two transactions to be sent to Sentry"
    );

    // Find transactions for first and second services
    let mut first_tx = None;
    let mut second_tx = None;

    for envelope in &envelopes {
        let tx = match envelope.items().next().unwrap() {
            sentry::protocol::EnvelopeItem::Transaction(tx) => tx.clone(),
            unexpected => panic!("Expected transaction, but got {:#?}", unexpected),
        };

        // Determine which service this transaction belongs to based on name
        match tx.name.as_deref() {
            Some("first_service") => first_tx = Some(tx),
            Some("second_service") => second_tx = Some(tx),
            name => panic!("Unexpected transaction name: {:?}", name),
        }
    }

    let first_tx = first_tx.expect("Missing first service transaction");
    let second_tx = second_tx.expect("Missing second service transaction");

    // Get first service trace ID and span ID
    let (first_trace_id, first_span_id) = match &first_tx.contexts.get("trace") {
        Some(sentry::protocol::Context::Trace(trace)) => (trace.trace_id, trace.span_id),
        _ => panic!("Missing trace context in first transaction"),
    };

    // Get second service trace ID and span ID
    let (second_trace_id, second_span_id, second_parent_span_id) =
        match &second_tx.contexts.get("trace") {
            Some(sentry::protocol::Context::Trace(trace)) => {
                (trace.trace_id, trace.span_id, trace.parent_span_id)
            }
            _ => panic!("Missing trace context in second transaction"),
        };

    // Verify the distributed trace - same trace ID, different span IDs
    assert_eq!(first_trace_id, second_trace_id, "Trace IDs should match");
    assert_ne!(
        first_span_id, second_span_id,
        "Span IDs should be different"
    );
    assert_eq!(
        second_parent_span_id,
        Some(first_span_id),
        "Second service's parent span ID should match first service's span ID"
    );
}

struct TestInjector<'a>(&'a mut HashMap<String, String>);

impl opentelemetry::propagation::Injector for TestInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

struct TestExtractor<'a>(&'a HashMap<String, String>);

impl opentelemetry::propagation::Extractor for TestExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}
