use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use crate::protocol;

use crate::Hub;

const MAX_SPANS: usize = 1_000;

// global API:

pub fn start_transaction() -> Transaction {
    Transaction::new()
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

struct TransactionInner {
    context: protocol::TraceContext,
    transaction: Mutex<protocol::Transaction<'static>>,
}

pub struct Transaction {
    inner: Arc<TransactionInner>,
}

impl Transaction {
    #[must_use = "a transaction must be explicitly closed via `finish()`"]
    pub fn new() -> Self {
        Self::continue_from_headers([])
    }

    #[must_use = "a transaction must be explicitly closed via `finish()`"]
    pub fn continue_from_headers<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
        headers: I,
    ) -> Self {
        let mut trace = None;
        for (k, v) in headers.into_iter() {
            if k == "sentry-trace" {
                trace = parse_sentry_trace(v);
            }
        }

        let (trace_id, parent_span_id) = match trace {
            Some(trace) => (trace.0, Some(trace.1)),
            None => (protocol::TraceId::default(), None),
        };

        let context = protocol::TraceContext {
            trace_id,
            parent_span_id,
            ..Default::default()
        };

        let transaction = Mutex::new(protocol::Transaction {
            ..Default::default()
        });

        Self {
            inner: Arc::new(TransactionInner {
                context,
                transaction,
            }),
        }
    }

    pub fn finish(self) {
        // TODO: maybe we should hold onto a ref of the client directly?
        if let Some(client) = Hub::current().client() {
            // NOTE: we expect this to always succeed, since all other places
            // use weak references (well, except if you are inside a `Span::finish`)
            if let Ok(inner) = Arc::try_unwrap(self.inner) {
                let mut transaction = inner.transaction.into_inner().unwrap();
                transaction.finish();
                transaction
                    .contexts
                    .insert("trace".into(), inner.context.into());

                let mut envelope = protocol::Envelope::new();
                envelope.add_item(transaction);

                client.send_envelope(envelope)
            }
        }
    }

    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self) -> Span {
        let span = protocol::Span {
            trace_id: self.inner.context.trace_id,
            parent_span_id: Some(self.inner.context.span_id),
            ..Default::default()
        };
        Span {
            transaction: Arc::downgrade(&self.inner),
            span,
        }
    }
}

pub struct Span {
    transaction: Weak<TransactionInner>,
    span: protocol::Span,
}

impl Span {
    pub fn iter_headers(&self) -> TraceHeadersIter {
        let trace = SentryTrace(self.span.trace_id, self.span.span_id, None);
        TraceHeadersIter {
            sentry_trace: Some(trace.to_string()),
        }
    }

    pub fn finish(mut self) {
        self.span.finish();
        if let Some(transaction) = self.transaction.upgrade() {
            let mut transaction = transaction.transaction.lock().unwrap();
            if transaction.spans.len() <= MAX_SPANS {
                transaction.spans.push(self.span);
            }
        }
    }

    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self) -> Span {
        let span = protocol::Span {
            trace_id: self.span.trace_id,
            parent_span_id: Some(self.span.span_id),
            ..Default::default()
        };
        Span {
            transaction: self.transaction.clone(),
            span,
        }
    }
}

pub struct TraceHeadersIter {
    sentry_trace: Option<String>,
}

impl Iterator for TraceHeadersIter {
    type Item = (&'static str, String);

    fn next(&mut self) -> Option<Self::Item> {
        self.sentry_trace.take().map(|st| ("sentry-trace", st))
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
