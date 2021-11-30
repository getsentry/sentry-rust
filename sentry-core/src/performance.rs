use crate::protocol;

use crate::Hub;

// global API:

pub fn start_transaction() -> Transaction {
    todo!()
}

pub fn start_span() -> Span {
    todo!()
}

// Hub API:

impl Hub {
    pub fn start_transaction() -> Transaction {
        todo!()
    }
}

// global API types:

pub struct Transaction {}

impl Transaction {
    pub fn continue_from_headers<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
        headers: I,
    ) -> Self {
        Transaction {}
    }

    pub fn finish(self) {}

    pub fn start_child() -> Span {
        todo!()
    }
}

pub struct Span {}

impl Span {
    pub fn to_sentry_trace(&self) -> String {
        format!("")
    }

    pub fn finish(self) {}

    pub fn start_child() -> Span {
        todo!()
    }
}

#[derive(Debug, PartialEq)]
struct SentryTrace(protocol::TraceId, protocol::SpanId, Option<bool>);

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

    Some(SentryTrace(trace_id, parent_span_id, parent_sampled))
}

impl std::fmt::Display for SentryTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.0, self.1)?;
        if let Some(sampled) = self.2 {
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
        use std::str::FromStr;
        let trace_id = protocol::TraceId::from_str("09e04486820349518ac7b5d2adbf6ba5").unwrap();
        let parent_trace_id = protocol::SpanId::from_str("9cf635fa5b870b3a").unwrap();

        let trace = parse_sentry_trace("09e04486820349518ac7b5d2adbf6ba5-9cf635fa5b870b3a-0");
        assert_eq!(
            trace,
            Some(SentryTrace(trace_id, parent_trace_id, Some(false)))
        );

        let trace = SentryTrace(Default::default(), Default::default(), None);
        let parsed = parse_sentry_trace(&format!("{}", trace));
        assert_eq!(parsed, Some(trace));
    }
}
