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

use crate::context::SentrySpanContext;

const SENTRY_TRACE_KEY: &str = "sentry-trace";

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
        injector.set(
            SENTRY_TRACE_KEY,
            format!(
                "{}-{}-{}",
                trace_id,
                span_id,
                if sampled { "1" } else { "0" }
            ),
        );
    }

    fn extract_with_context(&self, ctx: &Context, extractor: &dyn Extractor) -> Context {
        if let Some(sentry_trace) = extractor.get(SENTRY_TRACE_KEY) {
            let sentry_ctx: Result<SentrySpanContext, _> = sentry_trace.parse();
            if let Ok(value) = sentry_ctx {
                return ctx.with_value(value);
            }
        }
        ctx.clone()
    }

    fn fields(&self) -> FieldIter<'_> {
        FieldIter::new(&*SENTRY_PROPAGATOR_FIELDS)
    }
}
