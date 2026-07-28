//! Module containing utilities for interacting with Sentry tracing headers.

use crate::protocol;

/// A container for distributed tracing metadata that can be extracted from e.g. the `sentry-trace`
/// HTTP header.
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct SentryTrace {
    pub(crate) trace_id: protocol::TraceId,
    pub(crate) span_id: protocol::SpanId,
    pub(crate) sampled: Option<bool>,
}

impl SentryTrace {
    /// Creates a new [`SentryTrace`] from the provided parameters
    pub fn new(
        trace_id: protocol::TraceId,
        span_id: protocol::SpanId,
        sampled: Option<bool>,
    ) -> Self {
        SentryTrace {
            trace_id,
            span_id,
            sampled,
        }
    }
}

fn parse_sentry_trace(header: &str) -> Option<SentryTrace> {
    let header = header.trim();
    let mut parts = header.splitn(3, '-');

    let trace_id = parts.next()?.parse().ok()?;
    let parent_span_id = parts.next()?.parse().ok()?;
    let parent_sampled = parts.next().and_then(|sampled| match sampled {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    });

    Some(SentryTrace::new(trace_id, parent_span_id, parent_sampled))
}

impl std::fmt::Display for SentryTrace {
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
pub fn parse<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(headers: I) -> Option<SentryTrace> {
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
    use std::str::FromStr as _;

    use super::*;

    #[test]
    fn parses_sentry_trace() {
        let trace_id = protocol::TraceId::from_str("09e04486820349518ac7b5d2adbf6ba5").unwrap();
        let parent_trace_id = protocol::SpanId::from_str("9cf635fa5b870b3a").unwrap();

        let trace = parse_sentry_trace("09e04486820349518ac7b5d2adbf6ba5-9cf635fa5b870b3a-0");
        assert_eq!(
            trace,
            Some(SentryTrace::new(trace_id, parent_trace_id, Some(false)))
        );

        let trace = SentryTrace::new(Default::default(), Default::default(), None);
        let parsed = parse_sentry_trace(&trace.to_string());
        assert_eq!(parsed, Some(trace));
    }
}
