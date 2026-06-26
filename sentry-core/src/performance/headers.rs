//! Module containing utilities for interacting with Sentry tracing headers.

use crate::protocol::{SpanId, TraceId};

/// The [trace propagation] context.
///
/// Contains the information necessary for propagating Sentry traces and continuing traces from
/// incoming requests.
///
/// The data stored in this struct can be parsed from and transmitted as `sentry-trace` and Sentry
/// baggage headers.
///
/// Note that the Rust SDK only partially supports trace propagation, certain features such as
/// [dynamic sampling] may be missing or incomplete.
///
/// [trace propagation]: https://develop.sentry.dev/sdk/foundations/trace-propagation/
/// [dynamic sampling]: https://develop.sentry.dev/sdk/foundations/trace-propagation/dynamic-sampling-context/
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct TracePropagationContext {
    pub(crate) trace_id: TraceId,
    pub(crate) span_id: SpanId,
    pub(crate) sampled: Option<bool>,
}

/// Deprecated alias for [`TracePropagationContext`] for backwards-compatibility.
#[deprecated = "Please use `TracePropagationContext` instead"]
pub type SentryTrace = TracePropagationContext;

impl TracePropagationContext {
    /// Creates a new [`TracePropagationContext`] from the provided parameters
    pub fn new(trace_id: TraceId, span_id: SpanId, sampled: Option<bool>) -> Self {
        TracePropagationContext {
            trace_id,
            span_id,
            sampled,
        }
    }
}

fn parse_sentry_trace(header: &str) -> Option<TracePropagationContext> {
    let header = header.trim();
    let mut parts = header.splitn(3, '-');

    let trace_id = parts.next()?.parse().ok()?;
    let parent_span_id = parts.next()?.parse().ok()?;
    let parent_sampled = parts.next().and_then(|sampled| match sampled {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    });

    Some(TracePropagationContext::new(
        trace_id,
        parent_span_id,
        parent_sampled,
    ))
}

impl std::fmt::Display for TracePropagationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.trace_id, self.span_id)?;
        if let Some(sampled) = self.sampled {
            write!(f, "-{}", if sampled { '1' } else { '0' })?;
        }
        Ok(())
    }
}

/// Extracts distributed tracing metadata from headers (or, generally, key-value pairs),
/// considering the values for `sentry-trace`.
pub fn parse<'a, I>(headers: I) -> Option<TracePropagationContext>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut trace = None;
    for (k, v) in headers.into_iter() {
        if k.eq_ignore_ascii_case("sentry-trace") {
            trace = parse_sentry_trace(v);
            break;
        }
    }
    trace
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sentry_trace() {
        let trace_id = "09e04486820349518ac7b5d2adbf6ba5".parse().unwrap();
        let parent_trace_id = "9cf635fa5b870b3a".parse().unwrap();

        let trace = parse_sentry_trace("09e04486820349518ac7b5d2adbf6ba5-9cf635fa5b870b3a-0");
        assert_eq!(
            trace,
            Some(TracePropagationContext::new(
                trace_id,
                parent_trace_id,
                Some(false)
            ))
        );

        let trace = TracePropagationContext::new(Default::default(), Default::default(), None);
        let parsed = parse_sentry_trace(&trace.to_string());
        assert_eq!(parsed, Some(trace));
    }
}
