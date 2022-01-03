use std::sync::Arc;
use std::sync::Mutex;

use crate::protocol;
use crate::{Client, Hub};

const MAX_SPANS: usize = 1_000;

// global API:

/// Start a new Performance Monitoring Transaction.
///
/// The transaction needs to be explicitly finished via [`Transaction::finish`],
/// otherwise it will be discarded.
/// The transaction itself also represents the root span in the span hierarchy.
/// Child spans can be started with the [`Transaction::start_child`] method.
pub fn start_transaction(ctx: TransactionContext) -> Transaction {
    let client = Hub::with_active(|hub| hub.client());
    Transaction::new(client, ctx)
}

// Hub API:

impl Hub {
    /// Start a new Performance Monitoring Transaction.
    ///
    /// See the global [`start_transaction`] for more documentation.
    pub fn start_transaction(&self, ctx: TransactionContext) -> Transaction {
        Transaction::new(self.client(), ctx)
    }
}

// "Context" Types:

/// The Transaction Context used to start a new Performance Monitoring Transaction.
///
/// The Transaction Context defines the metadata for a Performance Monitoring
/// Transaction, and also the connection point for distributed tracing.
#[derive(Debug)]
pub struct TransactionContext {
    name: String,
    op: String,
    trace_id: protocol::TraceId,
    parent_span_id: Option<protocol::SpanId>,
}

impl TransactionContext {
    /// Creates a new Transaction Context with the given `name` and `op`.
    ///
    /// See <https://docs.sentry.io/platforms/native/enriching-events/transaction-name/>
    /// for an explanation of a Transaction's `name`, and
    /// <https://develop.sentry.dev/sdk/performance/span-operations/> for conventions
    /// around an `operation`'s value.
    ///
    /// See also the [`TransactionContext::continue_from_headers`] function that
    /// can be used for distributed tracing.
    #[must_use = "this must be used with `start_transaction`"]
    pub fn new(name: &str, op: &str) -> Self {
        Self::continue_from_headers(name, op, [])
    }

    /// Creates a new Transaction Context based on the distributed tracing `headers`.
    ///
    /// The `headers` in particular need to include the `sentry-trace` header,
    /// which is used to associate the transaction with a distributed trace.
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

    // TODO: `sampled` flag
}

// global API types:

/// A wrapper that groups a [`Transaction`] and a [`Span`] together.
#[derive(Clone, Debug)]
pub enum TransactionOrSpan {
    /// A [`Transaction`].
    Transaction(Transaction),
    /// A [`Span`].
    Span(Span),
}

impl From<Transaction> for TransactionOrSpan {
    fn from(transaction: Transaction) -> Self {
        Self::Transaction(transaction)
    }
}

impl From<Span> for TransactionOrSpan {
    fn from(span: Span) -> Self {
        Self::Span(span)
    }
}

impl TransactionOrSpan {
    /// Returns the headers needed for distributed tracing.
    pub fn iter_headers(&self) -> TraceHeadersIter {
        match self {
            TransactionOrSpan::Transaction(transaction) => transaction.iter_headers(),
            TransactionOrSpan::Span(span) => span.iter_headers(),
        }
    }

    /// Starts a new child Span with the given `op` and `description`.
    ///
    /// The span must be explicitly finished via [`Span::finish`], as it will
    /// otherwise not be sent to Sentry.
    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self, op: &str, description: &str) -> Span {
        match self {
            TransactionOrSpan::Transaction(transaction) => transaction.start_child(op, description),
            TransactionOrSpan::Span(span) => span.start_child(op, description),
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

    /// Finishes the Transaction/Span.
    ///
    /// This records the end timestamp and either sends the inner [`Transaction`]
    /// directly to Sentry, or adds the [`Span`] to its transaction.
    pub fn finish(self) {
        match self {
            TransactionOrSpan::Transaction(transaction) => transaction.finish(),
            TransactionOrSpan::Span(span) => span.finish(),
        }
    }
}

#[derive(Debug)]
struct TransactionInner {
    client: Option<Arc<Client>>,
    context: protocol::TraceContext,
    transaction: Option<protocol::Transaction<'static>>,
}

type TransactionArc = Arc<Mutex<TransactionInner>>;

/// A running Performance Monitoring Transaction.
///
/// The transaction needs to be explicitly finished via [`Transaction::finish`],
/// otherwise neither the transaction nor any of its child spans will be sent
/// to Sentry.
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

    /// Returns the headers needed for distributed tracing.
    pub fn iter_headers(&self) -> TraceHeadersIter {
        let inner = self.inner.lock().unwrap();
        let trace = SentryTrace(inner.context.trace_id, inner.context.span_id, None);
        TraceHeadersIter {
            sentry_trace: Some(trace.to_string()),
        }
    }

    /// Finishes the Transaction.
    ///
    /// This records the end timestamp and sends the transaction together with
    /// all finished child spans to Sentry.
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

    /// Starts a new child Span with the given `op` and `description`.
    ///
    /// The span must be explicitly finished via [`Span::finish`].
    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self, op: &str, description: &str) -> Span {
        let inner = self.inner.lock().unwrap();
        let span = protocol::Span {
            trace_id: inner.context.trace_id,
            parent_span_id: Some(inner.context.span_id),
            op: Some(op.into()),
            description: if description.is_empty() {
                None
            } else {
                Some(description.into())
            },
            ..Default::default()
        };
        Span {
            transaction: Arc::clone(&self.inner),
            span,
        }
    }
}

/// A running Performance Monitoring Span.
///
/// The span needs to be explicitly finished via [`Span::finish`], otherwise it
/// will not be sent to Sentry.
#[derive(Clone, Debug)]
pub struct Span {
    transaction: TransactionArc,
    span: protocol::Span,
}

impl Span {
    /// Returns the headers needed for distributed tracing.
    pub fn iter_headers(&self) -> TraceHeadersIter {
        let trace = SentryTrace(self.span.trace_id, self.span.span_id, None);
        TraceHeadersIter {
            sentry_trace: Some(trace.to_string()),
        }
    }

    /// Finishes the Span.
    ///
    /// This will record the end timestamp and add the span to the transaction
    /// in which it was started.
    pub fn finish(mut self) {
        self.span.finish();
        let mut inner = self.transaction.lock().unwrap();
        if let Some(transaction) = inner.transaction.as_mut() {
            if transaction.spans.len() <= MAX_SPANS {
                transaction.spans.push(self.span);
            }
        }
    }

    /// Starts a new child Span with the given `op` and `description`.
    ///
    /// The span must be explicitly finished via [`Span::finish`].
    #[must_use = "a span must be explicitly closed via `finish()`"]
    pub fn start_child(&self, op: &str, description: &str) -> Span {
        let span = protocol::Span {
            trace_id: self.span.trace_id,
            parent_span_id: Some(self.span.span_id),
            op: Some(op.into()),
            description: if description.is_empty() {
                None
            } else {
                Some(description.into())
            },
            ..Default::default()
        };
        Span {
            transaction: self.transaction.clone(),
            span,
        }
    }
}

/// An Iterator over HTTP header names and values needed for distributed tracing.
///
/// This currently only yields the `sentry-trace` header, but other headers
/// may be added in the future.
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
