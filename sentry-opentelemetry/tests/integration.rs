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

#[test]
fn test_creates_distributed_trace() {
    let transport = shared::init_sentry(1.0); // Sample all spans

    // Set up OpenTelemetry
    global::set_text_map_propagator(SentryPropagator::new());
    let tracer_provider = SdkTracerProvider::builder()
        .with_span_processor(SentrySpanProcessor::new())
        .build();
    let tracer = tracer_provider.tracer("test".to_string());

    // Create a "first service" span
    let first_service_span = tracer.start("first_service");
    let first_service_ctx = Context::current_with_span(first_service_span);

    // Simulate passing the context to another service by extracting and injecting e.g. HTTP
    // headers
    let propagator = SentryPropagator::new();
    let mut headers = HashMap::new();
    propagator.inject_context(&first_service_ctx, &mut TestInjector(&mut headers));

    // End the first service span
    first_service_ctx.span().end();

    // Check that the first service sent data to Sentry
    let first_envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(first_envelopes.len(), 1);

    let first_tx = match first_envelopes[0].items().next().unwrap() {
        sentry::protocol::EnvelopeItem::Transaction(tx) => tx.clone(),
        _ => panic!("Expected transaction"),
    };

    // Get first service trace ID and span ID
    let (first_trace_id, first_span_id) = match &first_tx.contexts.get("trace") {
        Some(sentry::protocol::Context::Trace(trace)) => (trace.trace_id, trace.span_id),
        _ => panic!("Missing trace context in first transaction"),
    };

    // Now simulate the second service receiving the headers and continuing the trace
    let second_service_ctx =
        propagator.extract_with_context(&Context::current(), &TestExtractor(&headers));

    // Create a second service span that continues the trace
    let second_service_span = tracer.start_with_context("second_service", &second_service_ctx);
    let second_service_ctx = second_service_ctx.with_span(second_service_span);

    // End the second service span
    second_service_ctx.span().end();

    // Check that the second service sent data to Sentry
    let second_envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(second_envelopes.len(), 1);

    let second_tx = match second_envelopes[0].items().next().unwrap() {
        sentry::protocol::EnvelopeItem::Transaction(tx) => tx.clone(),
        _ => panic!("Expected transaction"),
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

#[test]
fn test_associates_event_with_span() {
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

    // Set child span as active on current thread
    let child_cx_guard = child_cx.attach();

    // Capture an event while the child span is active
    sentry::capture_message("Test message", sentry::Level::Error);

    // End the spans
    drop(child_cx_guard);
    cx.span().end();

    // Capture the event and spans
    let envelopes = transport.fetch_and_clear_envelopes();

    // Find event and transaction
    let mut transaction: Option<Transaction> = None;
    let mut span_id: Option<String> = None;

    let mut trace_id_from_event: Option<String> = None;
    let mut span_id_from_event: Option<String> = None;

    for envelope in &envelopes {
        for item in envelope.items() {
            match item {
                sentry::protocol::EnvelopeItem::Event(event) => {
                    trace_id_from_event = event.contexts.get("trace").and_then(|c| match c {
                        sentry::protocol::Context::Trace(trace) => Some(trace.trace_id.to_string()),
                        _ => unreachable!(),
                    });
                    span_id_from_event = event.contexts.get("trace").and_then(|c| match c {
                        sentry::protocol::Context::Trace(trace) => Some(trace.span_id.to_string()),
                        _ => unreachable!(),
                    });
                }
                sentry::protocol::EnvelopeItem::Transaction(tx) => {
                    transaction = Some(tx.clone());
                    tx.spans.iter().for_each(|span| {
                        span_id = Some(span.span_id.to_string());
                    });
                }
                _ => (),
            }
        }
    }

    let transaction = transaction.expect("Should have a transaction");
    let span_id = span_id.expect("Transaction should have a child span");

    let trace_id_from_event = trace_id_from_event.expect("Event should have a trace ID");
    let span_id_from_event = span_id_from_event.expect("Event should have a span ID");

    // Verify that the transaction ID and span ID in the event match with the transaction and span
    assert_eq!(
        {
            let context = transaction.contexts.get("trace").unwrap().clone();
            match context {
                sentry::protocol::Context::Trace(context) => context.trace_id.to_string(),
                _ => unreachable!(),
            }
        },
        trace_id_from_event
    );
    assert_eq!(span_id, span_id_from_event);
}
