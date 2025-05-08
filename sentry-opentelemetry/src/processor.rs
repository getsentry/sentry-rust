//! An OpenTelemetry [SpanProcessor](https://opentelemetry.io/docs/specs/otel/trace/sdk/#span-processor) for Sentry.
//!
//! [`SentrySpanProcessor`] allows the Sentry Rust SDK to integrate with OpenTelemetry.
//! It transforms OpenTelemetry spans into Sentry transactions/spans and sends them to Sentry.
//!
//! # Configuration
//!
//! Unless you have no need for distributed tracing, this should be used together with [`crate::propagator::SentryPropagator`]. An example of
//! setting up both is provided in the [crate-level documentation](../).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use opentelemetry::global::ObjectSafeSpan;
use opentelemetry::trace::{get_active_span, SpanId};
use opentelemetry::Context;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{Span, SpanData, SpanProcessor};

use opentelemetry_sdk::Resource;
use sentry_core::SentryTrace;
use sentry_core::{TransactionContext, TransactionOrSpan};

use crate::converters::{
    convert_event, convert_span_id, convert_span_kind, convert_span_status, convert_trace_id,
    convert_value,
};

/// A mapping from Sentry span IDs to Sentry spans/transactions.
/// Sentry spans are created with the same SpanId as the corresponding OTEL span, so this is used
/// to track OTEL spans across start/end calls.
type SpanMap = Arc<Mutex<HashMap<sentry_core::protocol::SpanId, TransactionOrSpan>>>;

/// An OpenTelemetry SpanProcessor that converts OTEL spans to Sentry spans/transactions and sends
/// them to Sentry.
#[derive(Debug, Clone)]
pub struct SentrySpanProcessor {
    span_map: SpanMap,
}

impl SentrySpanProcessor {
    /// Creates a new `SentrySpanProcessor`.
    pub fn new() -> Self {
        sentry_core::configure_scope(|scope| {
            // Associate Sentry events with the correct span and trace.
            // This works as long as all Sentry spans/transactions are managed exclusively through OTEL APIs.
            scope.add_event_processor(|mut event| {
                get_active_span(|otel_span| {
                    let (span_id, trace_id) = (
                        convert_span_id(&otel_span.span_context().span_id()),
                        convert_trace_id(&otel_span.span_context().trace_id()),
                    );

                    if let Some(sentry_core::protocol::Context::Trace(trace_context)) =
                        event.contexts.get_mut("trace")
                    {
                        trace_context.trace_id = trace_id;
                        trace_context.span_id = span_id;
                    } else {
                        event.contexts.insert(
                            "trace".into(),
                            sentry_core::protocol::TraceContext {
                                span_id,
                                trace_id,
                                ..Default::default()
                            }
                            .into(),
                        );
                    }
                });
                Some(event)
            });
        });
        Self {
            span_map: Default::default(),
        }
    }
}

impl Default for SentrySpanProcessor {
    /// Creates a default `SentrySpanProcessor`.
    fn default() -> Self {
        Self::new()
    }
}

impl SpanProcessor for SentrySpanProcessor {
    fn on_start(&self, span: &mut Span, ctx: &Context) {
        let span_id = span.span_context().span_id();
        let trace_id = span.span_context().trace_id();

        let mut span_map = self.span_map.lock().unwrap();

        let mut span_description = String::new();
        let mut span_op = String::new();
        let mut span_start_timestamp = SystemTime::now();
        let mut parent_sentry_span = None;
        if let Some(data) = span.exported_data() {
            span_description = data.name.to_string();
            span_op = span_description.clone(); // TODO: infer this from OTEL span attributes
            span_start_timestamp = data.start_time;
            if data.parent_span_id != SpanId::INVALID {
                parent_sentry_span = span_map.get(&convert_span_id(&data.parent_span_id));
            };
        }
        let span_description = span_description.as_str();
        let span_op = span_op.as_str();

        let sentry_span = {
            if let Some(parent_sentry_span) = parent_sentry_span {
                // continue local trace
                TransactionOrSpan::Span(parent_sentry_span.start_child_with_details(
                    span_op,
                    span_description,
                    convert_span_id(&span_id),
                    span_start_timestamp,
                ))
            } else {
                let sentry_ctx = {
                    if let Some(sentry_trace) = ctx.get::<SentryTrace>() {
                        // continue remote trace
                        TransactionContext::continue_from_sentry_trace(
                            span_description,
                            span_op,
                            sentry_trace,
                        )
                    } else {
                        // start a new trace
                        TransactionContext::new_with_details(
                            span_description,
                            span_op,
                            convert_trace_id(&trace_id),
                            Some(convert_span_id(&span_id)),
                            None,
                        )
                    }
                };
                let tx =
                    sentry_core::start_transaction_with_timestamp(sentry_ctx, span_start_timestamp);
                TransactionOrSpan::Transaction(tx)
            }
        };
        span_map.insert(convert_span_id(&span_id), sentry_span);
    }

    fn on_end(&self, data: SpanData) {
        let span_id = data.span_context.span_id();

        let mut span_map = self.span_map.lock().unwrap();

        let Some(sentry_span) = span_map.remove(&convert_span_id(&span_id)) else {
            return;
        };

        sentry_span.set_data("otel.kind", convert_span_kind(data.span_kind));
        for attribute in data.attributes {
            sentry_span.set_data(attribute.key.as_str(), convert_value(attribute.value));
        }
        // TODO: read OTEL semantic convention span attributes and map them to the appropriate
        // Sentry span attributes/context values

        for event in data.events {
            sentry_core::capture_event(convert_event(&event));
        }

        sentry_span.set_status(convert_span_status(&data.status));
        sentry_span.finish_with_timestamp(data.end_time);
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }

    fn set_resource(&mut self, resource: &Resource) {
        sentry_core::configure_scope(|scope| {
            let otel_context = sentry_core::protocol::OtelContext {
                resource: resource
                    .iter()
                    .map(|(key, value)| (key.as_str().into(), convert_value(value.clone())))
                    .collect(),
                ..Default::default()
            };
            scope.set_context("otel", sentry_core::protocol::Context::from(otel_context));
        });
    }
}
