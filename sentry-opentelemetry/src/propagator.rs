//! An OpenTelemetry [Propagator](https://opentelemetry.io/docs/specs/otel/context/api-propagators/) for Sentry.
//!
//! [`SentryPropagator`] serves two purposes:
//! - extracts incoming Sentry tracing metadata from incoming traces, and stores it in
//!   [`opentelemetry::baggage::Baggage`]. This information can then be used by
//!   [`crate::processor::SentrySpanProcessor`] to achieve distributed tracing.
//! - injects Sentry tracing metadata in outgoing traces. This information can be used by
//!   downstream Sentry SDKs to achieve distributed tracing.
//!
//! # Configuration
//!
//! This should be used together with [`crate::processor::SentrySpanProcessor`]. An example of
//! setting up both is provided in the [crate-level documentation](../).

use std::sync::LazyLock;

use opentelemetry::{
    propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
    trace::TraceContextExt,
    Context, SpanId, TraceId,
};
use sentry_core::parse_headers;
use sentry_core::SentryTrace;

use crate::converters::{convert_span_id, convert_trace_id};

const SENTRY_TRACE_KEY: &str = "sentry-trace";

// list of headers used in the inject operation
static SENTRY_PROPAGATOR_FIELDS: LazyLock<[String; 1]> =
    LazyLock::new(|| [SENTRY_TRACE_KEY.to_owned()]);

/// An OpenTelemetry Propagator that injects and extracts Sentry's tracing headers to achieve
/// distributed tracing.
#[derive(Debug, Copy, Clone)]
pub struct SentryPropagator {}

impl SentryPropagator {
    /// Creates a new `SentryPropagator`
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SentryPropagator {
    /// Creates a default `SentryPropagator`.
    fn default() -> Self {
        Self::new()
    }
}

impl TextMapPropagator for SentryPropagator {
    fn inject_context(&self, ctx: &Context, injector: &mut dyn Injector) {
        let trace_id = ctx.span().span_context().trace_id();
        let span_id = ctx.span().span_context().span_id();
        let sampled = ctx.span().span_context().is_sampled();
        if trace_id == TraceId::INVALID || span_id == SpanId::INVALID {
            return;
        }
        let sentry_trace = SentryTrace::new(
            convert_trace_id(&trace_id),
            convert_span_id(&span_id),
            Some(sampled),
        );
        injector.set(SENTRY_TRACE_KEY, sentry_trace.to_string());
    }

    fn extract_with_context(&self, ctx: &Context, extractor: &dyn Extractor) -> Context {
        let keys = extractor.keys();
        let pairs = keys
            .iter()
            .filter_map(|&key| extractor.get(key).map(|value| (key, value)));
        if let Some(sentry_trace) = parse_headers(pairs) {
            return ctx.with_value(sentry_trace);
        }
        ctx.clone()
    }

    fn fields(&self) -> FieldIter<'_> {
        FieldIter::new(&*SENTRY_PROPAGATOR_FIELDS)
    }
}
