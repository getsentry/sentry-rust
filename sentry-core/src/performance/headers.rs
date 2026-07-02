//! Module containing utilities for interacting with Sentry tracing headers.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::protocol::{SpanId, TraceId};

/// A key-value header pair.
type Header<'h> = (&'h str, &'h str);

/// The [trace propagation] context.
///
/// Contains the information necessary for propagating Sentry traces and continuing traces from
/// incoming requests.
///
/// The data stored in this struct can be parsed from and transmitted as `sentry-trace` headers.
///
/// Note that the Rust SDK only partially supports trace propagation, certain features such as
/// [dynamic sampling] may be missing or incomplete.
///
/// [trace propagation]: https://develop.sentry.dev/sdk/foundations/trace-propagation/
/// [dynamic sampling]: https://develop.sentry.dev/sdk/foundations/trace-propagation/dynamic-sampling-context/
#[derive(Debug, PartialEq, Clone, Default)]
pub struct TracePropagationContext {
    pub(crate) trace_id: TraceId,
    pub(crate) span_id: SpanId,
    pub(super) sampled: Option<bool>,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
/// Error type returned by [`TracePropagationContext::try_from_headers`].
pub enum HeaderParseError {
    /// The `sentry-trace` header was missing.
    Missing,
    /// There was a `sentry-trace` header, but it was invalid.
    Invalid,
}

/// A container for `sentry-trace` data.
#[deprecated = "Please use `TracePropagationContext` instead"]
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct SentryTrace {
    trace_id: TraceId,
    span_id: SpanId,
    sampled: Option<bool>,
}

impl TracePropagationContext {
    /// Creates a new [`TracePropagationContext`] from the provided parameters
    pub fn new(trace_id: TraceId, span_id: SpanId) -> Self {
        TracePropagationContext {
            trace_id,
            span_id,
            sampled: None,
        }
    }

    /// Set the sampling decision on `self`.
    pub fn with_sampled(self, sampled: bool) -> Self {
        let sampled = Some(sampled);
        Self { sampled, ..self }
    }

    /// Computes the `sentry-trace` header for this [`TracePropagationContext`].
    pub fn sentry_trace_header(&self) -> String {
        let Self {
            trace_id,
            span_id,
            sampled,
        } = self;

        let sampled_suffix = sampled
            .map(|sampled| format!("-{}", if sampled { "1" } else { "0" }))
            .unwrap_or_default();

        format!("{trace_id}-{span_id}{sampled_suffix}")
    }

    /// Attempt to parse a list of Sentry headers into [`TracePropagationContext`].
    ///
    /// The parsing will fail if there is no valid `sentry-trace` header.
    pub fn try_from_headers<'a, I>(headers: I) -> Result<Self, HeaderParseError>
    where
        I: IntoIterator<Item = Header<'a>>,
    {
        let mut context_result = Err(HeaderParseError::Missing);

        for (header, value) in headers {
            if header.eq_ignore_ascii_case("sentry-trace") {
                // Parse the header, falling back to the previous header value if Ok (headers not
                // guaranteed unique), only falling back to invalid error if there's no prev value.
                context_result = TracePropagationContext::from_sentry_trace(value)
                    .map_or(context_result, Ok)
                    .map_err(|_| HeaderParseError::Invalid);
            }
        }

        context_result
    }

    /// Attempts to construct a [`TracePropagationContext`] from the given Sentry trace header.
    ///
    /// Returns [`None`] if the header cannot be parsed.
    fn from_sentry_trace(header: &str) -> Option<Self> {
        let header = header.trim();
        let mut parts = header.splitn(3, '-');

        let trace_id = parts.next()?.parse().ok()?;
        let span_id = parts.next()?.parse().ok()?;
        let sampled = parts.next().and_then(|sampled| match sampled {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        });

        Some(Self {
            trace_id,
            span_id,
            sampled,
        })
    }
}

/// Extracts distributed tracing metadata from headers (or, generally, key-value pairs),
/// considering the values for `sentry-trace`.
#[deprecated = "use TracePropagationContext::try_from_headers instead"]
#[expect(deprecated, reason = "backwards-compatible function")]
pub fn parse_sentry_trace_header<'a, I>(headers: I) -> Option<SentryTrace>
where
    I: IntoIterator<Item = Header<'a>>,
{
    let TracePropagationContext {
        trace_id,
        span_id,
        sampled,
    } = TracePropagationContext::try_from_headers(headers).ok()?;

    Some(SentryTrace {
        trace_id,
        span_id,
        sampled,
    })
}

impl Display for HeaderParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let msg = match self {
            HeaderParseError::Missing => "missing",
            HeaderParseError::Invalid => "invalid",
        };

        write!(f, "{msg} sentry-trace header")
    }
}

impl Error for HeaderParseError {}

#[expect(deprecated, reason = "backwards-compatible impl")]
impl SentryTrace {
    /// Creates a new [`SentryTrace`] from the provided parameters
    pub fn new(trace_id: TraceId, span_id: SpanId, sampled: Option<bool>) -> Self {
        Self {
            trace_id,
            span_id,
            sampled,
        }
    }
}

#[expect(deprecated, reason = "backwards-compatible impl")]
impl From<SentryTrace> for TracePropagationContext {
    fn from(trace: SentryTrace) -> Self {
        Self {
            trace_id: trace.trace_id,
            span_id: trace.span_id,
            sampled: trace.sampled,
        }
    }
}

#[expect(deprecated, reason = "backwards-compatible impl")]
impl std::fmt::Display for SentryTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.trace_id, self.span_id)?;
        if let Some(sampled) = self.sampled {
            write!(f, "-{}", if sampled { '1' } else { '0' })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sentry_trace() {
        let trace_id = "09e04486820349518ac7b5d2adbf6ba5".parse().unwrap();
        let parent_trace_id = "9cf635fa5b870b3a".parse().unwrap();

        let trace = TracePropagationContext::try_from_headers([(
            "sentry-trace",
            "09e04486820349518ac7b5d2adbf6ba5-9cf635fa5b870b3a-0",
        )])
        .expect("should parse successfully");
        assert_eq!(
            trace,
            TracePropagationContext {
                trace_id,
                span_id: parent_trace_id,
                sampled: Some(false),
            }
        );

        let trace = TracePropagationContext::new(Default::default(), Default::default());
        let parsed = TracePropagationContext::try_from_headers([(
            "sentry-trace",
            trace.sentry_trace_header().as_str(),
        )])
        .expect("should parse successfully");
        assert_eq!(parsed, trace);
    }
}
