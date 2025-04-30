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
    baggage::{Baggage, BaggageExt},
    propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
    trace::TraceContextExt,
    Context, SpanId, TraceId,
};

const SENTRY_TRACE_HEADER: &str = "sentry-trace";
const SENTRY_TRACE_ID_KEY: &str = "sentry-trace-id";
const SENTRY_SPAN_ID_KEY: &str = "sentry-span-id";
const SENTRY_SAMPLED_KEY: &str = "sentry-sampled";

pub(crate) fn extract_trace_data(
    cx: &Context,
) -> Option<(
    sentry_core::protocol::TraceId,
    sentry_core::protocol::SpanId,
    Option<bool>,
)> {
    Some((
        cx.baggage()
            .get(SENTRY_TRACE_ID_KEY)?
            .to_string()
            .parse()
            .ok()?,
        cx.baggage()
            .get(SENTRY_SPAN_ID_KEY)?
            .to_string()
            .parse()
            .ok()?,
        cx.baggage()
            .get(SENTRY_SAMPLED_KEY)
            .and_then(|sampled| match sampled.as_str() {
                "0" => Some(false),
                "1" => Some(true),
                _ => None,
            }),
    ))
}

static SENTRY_PROPAGATOR_FIELDS: LazyLock<[String; 1]> =
    LazyLock::new(|| [SENTRY_TRACE_HEADER.to_owned()]);

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
        let span_id = ctx.span().span_context().span_id();
        let trace_id = ctx.span().span_context().trace_id();
        let sampled = ctx.span().span_context().is_sampled();
        if span_id == SpanId::INVALID || trace_id == TraceId::INVALID {
            return;
        }
        injector.set(
            SENTRY_TRACE_HEADER,
            format!(
                "{}-{}-{}",
                trace_id,
                span_id,
                if sampled { "1" } else { "0" }
            ),
        );
    }

    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
        if let Some(sentry_trace) = extractor.get(SENTRY_TRACE_HEADER) {
            let sentry_trace: Vec<&str> = sentry_trace.split("-").collect();
            // at least trace ID and span ID need to be present
            if sentry_trace.len() < 2 {
                return cx.clone();
            }
            let mut baggage = Baggage::new();
            baggage.insert(SENTRY_TRACE_ID_KEY, sentry_trace[0].to_owned());
            baggage.insert(SENTRY_SPAN_ID_KEY, sentry_trace[1].to_owned());
            sentry_trace.get(2).inspect(|sampled| {
                baggage.insert(SENTRY_SAMPLED_KEY, sampled.to_string());
            });
            return cx.with_baggage(baggage);
        }
        cx.clone()
    }

    fn fields(&self) -> FieldIter<'_> {
        FieldIter::new(&*SENTRY_PROPAGATOR_FIELDS)
    }
}
