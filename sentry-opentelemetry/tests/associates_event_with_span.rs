mod shared;

use opentelemetry::{
    global,
    trace::{Tracer, TracerProvider},
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use sentry_core::protocol::Transaction;
use sentry_opentelemetry::{SentryPropagator, SentrySpanProcessor};

#[test]
fn test_associates_event_with_span() {
    let transport = shared::init_sentry(1.0); // Sample all spans

    // Set up OpenTelemetry
    global::set_text_map_propagator(SentryPropagator::new());
    let tracer_provider = SdkTracerProvider::builder()
        .with_span_processor(SentrySpanProcessor::new())
        .build();
    let tracer = tracer_provider.tracer("test".to_string());

    // Create root span and execute test within it
    tracer.in_span("root_span", |_| {
        // Create child span and execute within it
        tracer.in_span("child_span", |_| {
            // Capture an event while the child span is active
            sentry::capture_message("Test message", sentry::Level::Error);
        });
    });

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
