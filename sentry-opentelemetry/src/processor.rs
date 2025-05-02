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
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use crate::context::SentrySpanContext;

use opentelemetry::global::ObjectSafeSpan;
use opentelemetry::trace::{get_active_span, SpanId};
use opentelemetry::{Context, KeyValue};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{Span, SpanData, SpanProcessor};

use opentelemetry_sdk::Resource;
use sentry_core::{TransactionContext, TransactionOrSpan};

use crate::converters::{
    convert_attributes, convert_span_id, convert_span_kind, convert_span_status, convert_trace_id,
    convert_value,
};

/// A mapping from Sentry span IDs to Sentry spans/transactions.
/// Sentry spans are created with the same SpanId as the corresponding OTEL span, so this is used
/// to track OTEL spans across start/end calls.
type SpanMap = Arc<Mutex<HashMap<sentry_core::protocol::SpanId, TransactionOrSpan>>>;

static SPAN_MAP: LazyLock<SpanMap> = LazyLock::new(Default::default);

/// An OpenTelemetry SpanProcessor that converts OTEL spans to Sentry spans/transactions and sends
/// them to Sentry.
#[derive(Debug, Clone)]
pub struct SentrySpanProcessor {}

impl SentrySpanProcessor {
    /// Creates a new `SentrySpanProcessor`.
    pub fn new() -> Self {
        sentry_core::configure_scope(|scope| {
            // Associate Sentry events with the correct span and trace.
            // This works as long as all Sentry spans/transactions are managed exclusively through OTEL APIs.
            scope.add_event_processor(|mut event| {
                get_active_span(|otel_span| {
                    let span_map = SPAN_MAP.lock().unwrap();

                    let span = span_map.get(&convert_span_id(&otel_span.span_context().span_id()));
                    if span.is_none() {
                        return;
                    }
                    let span = span.unwrap();

                    let (span_id, trace_id) = match span {
                        TransactionOrSpan::Transaction(transaction) => (
                            transaction.get_trace_context().span_id,
                            transaction.get_trace_context().trace_id,
                        ),
                        TransactionOrSpan::Span(span) => {
                            (span.get_span_id(), span.get_trace_context().trace_id)
                        }
                    };

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
        Self {}
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
        let span_data = span.exported_data();
        // TODO: infer these from OTEL span attributes
        let span_description = span_data
            .as_ref()
            .map(|data| data.name.as_ref())
            .unwrap_or("");
        let span_op = span_description;
        let span_start_timestamp = span_data
            .as_ref()
            .map(|data| data.start_time)
            .unwrap_or_else(SystemTime::now);

        let mut span_map = SPAN_MAP.lock().unwrap();

        let parent_sentry_span = {
            span_data
                .as_ref()
                .map(|data| &data.parent_span_id)
                .filter(|id| **id != SpanId::INVALID)
                .and_then(|id| span_map.get(&convert_span_id(id)))
        };

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
                let distributed_trace_data = ctx.get::<SentrySpanContext>();
                if let Some(SentrySpanContext {
                    trace_id,
                    span_id: parent_span_id,
                    sampled,
                }) = distributed_trace_data
                {
                    // continue remote trace
                    let mut sentry_ctx = TransactionContext::new_with_details(
                        span_description,
                        span_op,
                        *trace_id,
                        Some(convert_span_id(&span_id)),
                        Some(*parent_span_id),
                    );
                    sentry_ctx.set_sampled(*sampled);
                    let tx = sentry_core::start_transaction_with_timestamp(
                        sentry_ctx,
                        span_start_timestamp,
                    );
                    TransactionOrSpan::Transaction(tx)
                } else {
                    // start a new trace
                    let sentry_ctx = TransactionContext::new_with_details(
                        span_description,
                        span_op,
                        convert_trace_id(&trace_id),
                        Some(convert_span_id(&span_id)),
                        None,
                    );
                    let tx = sentry_core::start_transaction_with_timestamp(
                        sentry_ctx,
                        span_start_timestamp,
                    );
                    TransactionOrSpan::Transaction(tx)
                }
            }
        };
        span_map.insert(convert_span_id(&span_id), sentry_span);
    }

    fn on_end(&self, data: SpanData) {
        let span_id = data.span_context.span_id();

        let mut span_map = SPAN_MAP.lock().unwrap();

        let sentry_span = span_map.remove(&convert_span_id(&span_id));
        if sentry_span.is_none() {
            return;
        }
        let sentry_span = sentry_span.unwrap();

        // TODO: read OTEL span events and convert them to Sentry breadcrumbs/events

        sentry_span.set_data("otel.kind", convert_span_kind(data.span_kind));
        for attribute in data.attributes {
            sentry_span.set_data(attribute.key.as_str(), convert_value(attribute.value));
        }
        // TODO: read OTEL semantic convention span attributes and map them to the appropriate
        // Sentry span attributes/context values
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
                resource: convert_attributes(
                    resource
                        .iter()
                        .map(|(key, value)| KeyValue::new(key.clone(), value.clone()))
                        .collect(),
                ),
                ..Default::default()
            };
            scope.set_context("otel", sentry_core::protocol::Context::from(otel_context));
        });
    }
}
