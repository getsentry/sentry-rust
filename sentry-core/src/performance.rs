use std::sync::Arc;
use std::sync::Mutex;

use crate::protocol;
use crate::{Client, Hub, Scope};

const MAX_SPANS: usize = 1_000;

// global API:

pub fn start_transaction(ctx: TransactionContext) -> Transaction {
    let client = Hub::with_active(|hub| hub.client());
    Transaction::new(client, ctx)
}

// Hub API:

impl Hub {
    pub fn start_transaction(&self, ctx: TransactionContext) -> Transaction {
        Transaction::new(self.client(), ctx)
    }
}

// "Context" Types:

#[derive(Debug)]
pub struct TransactionContext {
    name: String,
    op: String,
    trace_id: protocol::TraceId,
    parent_span_id: Option<protocol::SpanId>,
}

impl TransactionContext {
    #[must_use = "this must be used with `start_transaction`"]
    pub fn new(name: &str, op: &str) -> Self {
        Self::continue_from_headers(name, op, [])
    }

    #[must_use = "this must be used with `start_transaction`"]
    pub fn continue_from_headers<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
        name: &str,
        op: &str,
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

        Self {
            name: name.into(),
            op: op.into(),
            trace_id,
            parent_span_id,
        }
    }
}

// global API types:

#[derive(Debug)]
pub enum TransactionOrSpan {
    Transaction(Transaction),
    Span(Span),
}

impl TransactionOrSpan {
    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self, op: &str) -> Span {
        match self {
            TransactionOrSpan::Transaction(transaction) => transaction.start_child(op),
            TransactionOrSpan::Span(span) => span.start_child(op),
        }
    }

    pub(crate) fn apply_to_event(&self, event: &mut protocol::Event<'_>) {
        if event.contexts.contains_key("trace") {
            return;
        }

        let context = match self {
            TransactionOrSpan::Transaction(transaction) => {
                transaction.inner.lock().unwrap().context.clone()
            }
            TransactionOrSpan::Span(span) => protocol::TraceContext {
                span_id: span.span.span_id,
                trace_id: span.span.trace_id,
                ..Default::default()
            },
        };
        event.contexts.insert("trace".into(), context.into());
    }
}

#[derive(Debug)]
struct TransactionInner {
    client: Option<Arc<Client>>,
    context: protocol::TraceContext,
    transaction: Option<protocol::Transaction<'static>>,
}

type TransactionArc = Arc<Mutex<TransactionInner>>;

#[derive(Clone, Debug)]
pub struct Transaction {
    inner: TransactionArc,
}

impl Transaction {
    fn new(client: Option<Arc<Client>>, ctx: TransactionContext) -> Self {
        let context = protocol::TraceContext {
            trace_id: ctx.trace_id,
            parent_span_id: ctx.parent_span_id,
            op: Some(ctx.op),
            ..Default::default()
        };

        let transaction = if client.is_some() {
            Some(protocol::Transaction {
                name: Some(ctx.name),
                ..Default::default()
            })
        } else {
            None
        };

        Self {
            inner: Arc::new(Mutex::new(TransactionInner {
                client,
                context,
                transaction,
            })),
        }
    }

    pub fn finish(self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(mut transaction) = inner.transaction.take() {
            if let Some(client) = inner.client.take() {
                transaction.finish();
                transaction
                    .contexts
                    .insert("trace".into(), inner.context.clone().into());

                let mut envelope = protocol::Envelope::new();
                envelope.add_item(transaction);

                client.send_envelope(envelope)
            }
        }
    }

    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self, op: &str) -> Span {
        let inner = self.inner.lock().unwrap();
        let span = protocol::Span {
            trace_id: inner.context.trace_id,
            parent_span_id: Some(inner.context.span_id),
            op: Some(op.into()),
            ..Default::default()
        };
        Span {
            transaction: Arc::clone(&self.inner),
            span,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Span {
    transaction: TransactionArc,
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
        let mut inner = self.transaction.lock().unwrap();
        if let Some(transaction) = inner.transaction.as_mut() {
            if transaction.spans.len() <= MAX_SPANS {
                transaction.spans.push(self.span);
            }
        }
    }

    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self, op: &str) -> Span {
        let span = protocol::Span {
            trace_id: self.span.trace_id,
            parent_span_id: Some(self.span.span_id),
            op: Some(op.into()),
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
