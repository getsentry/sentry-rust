use std::str::FromStr;

use sentry_core::protocol::{SpanId, TraceId};

/// Represents the data stored in Sentry's `sentry-trace` container for
/// [distributed tracing](https://develop.sentry.dev/sdk/telemetry/traces/distributed-tracing/).
///
/// The [`crate::SentryPropagator`] extracts this data and stores it as [`SentrySpanContext`] on the
/// [`opentelemetry::Context`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct SentrySpanContext {
    pub(crate) trace_id: TraceId,
    pub(crate) span_id: SpanId,
    pub(crate) sampled: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ParseContextError;

impl FromStr for SentrySpanContext {
    type Err = ParseContextError;

    fn from_str(sentry_trace: &str) -> Result<Self, Self::Err> {
        let sentry_trace: Vec<&str> = sentry_trace.split("-").collect();
        // at least trace ID and span ID need to be present
        if sentry_trace.len() < 2 {
            return Err(ParseContextError {});
        }
        let trace_id = sentry_trace[0].parse().map_err(|_| ParseContextError {})?;
        let span_id = sentry_trace[1].parse().map_err(|_| ParseContextError {})?;
        let sampled = sentry_trace.get(2).and_then(|sampled| match *sampled {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        });

        Ok(SentrySpanContext {
            trace_id,
            span_id,
            sampled,
        })
    }
}
