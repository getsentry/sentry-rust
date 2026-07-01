//! Contains context types for transactions.

use sentry_types::protocol::v7::TraceContext;

use super::headers::TracePropagationContext;
#[expect(deprecated, reason = "backwards-compatibility")]
use super::SentryTrace;
use super::TransactionOrSpan;
use crate::protocol::{OrganizationId, SpanId, TraceId};

/// Arbitrary data passed by the caller, when starting a transaction.
///
/// May be inspected by the user in the `traces_sampler` callback, if set.
///
/// Represents arbitrary JSON data, the top level of which must be a map.
pub type CustomTransactionContext = serde_json::Map<String, serde_json::Value>;

/// The Transaction Context used to start a new Performance Monitoring Transaction.
///
/// The Transaction Context defines the metadata for a Performance Monitoring
/// Transaction, and also the connection point for distributed tracing.
#[derive(Debug, Clone)]
pub struct TransactionContext {
    #[cfg_attr(not(feature = "client"), allow(dead_code))]
    name: String,
    op: String,
    trace_id: TraceId,
    parent_span_id: Option<SpanId>,
    span_id: SpanId,
    sampled: Option<bool>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by future strict trace continuation")
    )]
    incoming_org_id: Option<OrganizationId>,
    custom: Option<CustomTransactionContext>,
}

impl TransactionContext {
    /// Creates a new Transaction Context with the given `name` and `op`. A random
    /// `trace_id` is assigned. Use [`TransactionContext::new_with_trace_id`] to
    /// specify a custom trace ID.
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
        Self::new_with_trace_id(name, op, TraceId::default())
    }

    /// Creates a new Transaction Context with the given `name`, `op`, and `trace_id`.
    ///
    /// See <https://docs.sentry.io/platforms/native/enriching-events/transaction-name/>
    /// for an explanation of a Transaction's `name`, and
    /// <https://develop.sentry.dev/sdk/performance/span-operations/> for conventions
    /// around an `operation`'s value.
    #[must_use = "this must be used with `start_transaction`"]
    pub fn new_with_trace_id(name: &str, op: &str, trace_id: TraceId) -> Self {
        Self {
            name: name.into(),
            op: op.into(),
            trace_id,
            parent_span_id: None,
            span_id: Default::default(),
            sampled: None,
            incoming_org_id: None,
            custom: None,
        }
    }

    /// Creates a new Transaction Context with the given `name`, `op`, `trace_id`, and
    /// possibly the given `span_id` and `parent_span_id`.
    ///
    /// See <https://docs.sentry.io/platforms/native/enriching-events/transaction-name/>
    /// for an explanation of a Transaction's `name`, and
    /// <https://develop.sentry.dev/sdk/performance/span-operations/> for conventions
    /// around an `operation`'s value.
    #[must_use = "this must be used with `start_transaction`"]
    pub fn new_with_details(
        name: &str,
        op: &str,
        trace_id: TraceId,
        span_id: Option<SpanId>,
        parent_span_id: Option<SpanId>,
    ) -> Self {
        let mut slf = Self::new_with_trace_id(name, op, trace_id);
        if let Some(span_id) = span_id {
            slf.span_id = span_id;
        }
        slf.parent_span_id = parent_span_id;
        slf
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
        TracePropagationContext::try_from_headers(headers)
            .map(|context| Self::continue_from_trace_propagation_context(name, op, &context, None))
            .unwrap_or_else(|_| Self {
                name: name.into(),
                op: op.into(),
                trace_id: Default::default(),
                parent_span_id: None,
                span_id: Default::default(),
                sampled: None,
                incoming_org_id: None,
                custom: None,
            })
    }

    /// Creates a new Transaction Context based on the provided distributed tracing data,
    /// optionally creating the `TransactionContext` with the provided `span_id`.
    #[deprecated = "use `TransactionContext::continue_from_trace_propagation_context` instead"]
    #[expect(deprecated, reason = "backwards-compatible method")]
    pub fn continue_from_sentry_trace(
        name: &str,
        op: &str,
        sentry_trace: &SentryTrace,
        span_id: Option<SpanId>,
    ) -> Self {
        let context = (*sentry_trace).into();
        Self::continue_from_trace_propagation_context(name, op, &context, span_id)
    }

    /// Creates a new Transaction Context based on the provided trace propagation context,
    /// optionally creating the `TransactionContext` with the provided `span_id`.
    pub fn continue_from_trace_propagation_context(
        name: &str,
        op: &str,
        context: &TracePropagationContext,
        span_id: Option<SpanId>,
    ) -> Self {
        let &TracePropagationContext {
            trace_id,
            span_id: context_span_id,
            sampled,
            org_id,
        } = context;

        Self {
            name: name.into(),
            op: op.into(),
            trace_id,
            parent_span_id: Some(context_span_id),
            sampled,
            incoming_org_id: org_id,
            span_id: span_id.unwrap_or_default(),
            custom: None,
        }
    }

    /// Creates a new Transaction Context based on an existing Span.
    ///
    /// This should be used when an independent computation is spawned on another
    /// thread and should be connected to the calling thread via a distributed
    /// tracing transaction.
    pub fn continue_from_span(name: &str, op: &str, span: Option<TransactionOrSpan>) -> Self {
        let span = match span {
            Some(span) => span,
            None => return Self::new(name, op),
        };

        let (trace_id, parent_span_id, sampled) = match span {
            TransactionOrSpan::Transaction(transaction) => {
                let inner = transaction.inner.lock().unwrap();
                (
                    inner.context.trace_id,
                    inner.context.span_id,
                    Some(inner.sampled),
                )
            }
            TransactionOrSpan::Span(span) => {
                let sampled = span.sampled;
                let span = span.span.lock().unwrap();
                (span.trace_id, span.span_id, Some(sampled))
            }
        };

        Self {
            name: name.into(),
            op: op.into(),
            trace_id,
            parent_span_id: Some(parent_span_id),
            span_id: SpanId::default(),
            sampled,
            incoming_org_id: None,
            custom: None,
        }
    }

    /// Set the sampling decision for this Transaction.
    ///
    /// This can be either an explicit boolean flag, or [`None`], which will fall
    /// back to use the configured `traces_sample_rate` option.
    pub fn set_sampled(&mut self, sampled: impl Into<Option<bool>>) {
        self.sampled = sampled.into();
    }

    /// Get the sampling decision for this Transaction.
    pub fn sampled(&self) -> Option<bool> {
        self.sampled
    }

    /// Get the name of this Transaction.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the operation of this Transaction.
    pub fn operation(&self) -> &str {
        &self.op
    }

    /// Get the Trace ID of this Transaction.
    pub fn trace_id(&self) -> TraceId {
        self.trace_id
    }

    /// Get the Span ID of this Transaction.
    pub fn span_id(&self) -> SpanId {
        self.span_id
    }

    /// Get the custom context of this Transaction.
    pub fn custom(&self) -> Option<&CustomTransactionContext> {
        self.custom.as_ref()
    }

    /// Update the custom context of this Transaction.
    ///
    /// For simply adding a key, use the `custom_insert` method.
    pub fn custom_mut(&mut self) -> &mut Option<CustomTransactionContext> {
        &mut self.custom
    }

    /// Inserts a key-value pair into the custom context.
    ///
    /// If the context did not have this key present, None is returned.
    ///
    /// If the context did have this key present, the value is updated, and the old value is
    /// returned.
    pub fn custom_insert(
        &mut self,
        key: String,
        value: serde_json::Value,
    ) -> Option<serde_json::Value> {
        // Get the custom context
        let mut custom = None;
        std::mem::swap(&mut self.custom, &mut custom);

        // Initialise the context, if not used yet
        let mut custom = custom.unwrap_or_default();

        // And set our key
        let existing_value = custom.insert(key, value);
        self.custom = Some(custom);
        existing_value
    }

    /// Creates a transaction context builder initialized with the given `name` and `op`.
    ///
    /// See <https://docs.sentry.io/platforms/native/enriching-events/transaction-name/>
    /// for an explanation of a Transaction's `name`, and
    /// <https://develop.sentry.dev/sdk/performance/span-operations/> for conventions
    /// around an `operation`'s value.
    #[must_use]
    pub fn builder(name: &str, op: &str) -> TransactionContextBuilder {
        TransactionContextBuilder {
            ctx: TransactionContext::new(name, op),
        }
    }

    /// Destructure `self` into the parts needed to initialize a transaction.
    pub(super) fn into_parts(self) -> TransactionContextParts {
        let Self {
            name,
            op,
            trace_id,
            parent_span_id,
            span_id,
            sampled,
            incoming_org_id: _,
            custom: _,
        } = self;

        let trace_context = TraceContext {
            span_id,
            trace_id,
            parent_span_id,
            op: Some(op),
            ..Default::default()
        };

        #[cfg(not(feature = "client"))]
        let _ = name;

        TransactionContextParts {
            #[cfg(feature = "client")]
            name,
            trace_context,
            sampled,
        }
    }
}

/// A transaction context builder created by [`TransactionContext::builder`].
pub struct TransactionContextBuilder {
    ctx: TransactionContext,
}

impl TransactionContextBuilder {
    /// Defines the name of the transaction.
    #[must_use]
    pub fn with_name(mut self, name: String) -> Self {
        self.ctx.name = name;
        self
    }

    /// Defines the operation of the transaction.
    #[must_use]
    pub fn with_op(mut self, op: String) -> Self {
        self.ctx.op = op;
        self
    }

    /// Defines the trace ID.
    #[must_use]
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.ctx.trace_id = trace_id;
        self
    }

    /// Defines a parent span ID for the created transaction.
    #[must_use]
    pub fn with_parent_span_id(mut self, parent_span_id: Option<SpanId>) -> Self {
        self.ctx.parent_span_id = parent_span_id;
        self
    }

    /// Defines the span ID to be used when creating the transaction.
    #[must_use]
    pub fn with_span_id(mut self, span_id: SpanId) -> Self {
        self.ctx.span_id = span_id;
        self
    }

    /// Defines whether the transaction will be sampled.
    #[must_use]
    pub fn with_sampled(mut self, sampled: Option<bool>) -> Self {
        self.ctx.sampled = sampled;
        self
    }

    /// Adds a custom key and value to the transaction context.
    #[must_use]
    pub fn with_custom(mut self, key: String, value: serde_json::Value) -> Self {
        self.ctx.custom_insert(key, value);
        self
    }

    /// Finishes building a transaction.
    pub fn finish(self) -> TransactionContext {
        self.ctx
    }
}

/// The type returned by [`TransactionContext::into_parts`].
pub(super) struct TransactionContextParts {
    #[cfg(feature = "client")]
    pub(super) name: String,
    pub(super) trace_context: TraceContext,
    pub(super) sampled: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continue_from_headers_stores_incoming_org_id() {
        let ctx = TransactionContext::continue_from_headers(
            "noop",
            "noop",
            [
                (
                    "sentry-trace",
                    "09e04486820349518ac7b5d2adbf6ba5-9cf635fa5b870b3a-1",
                ),
                ("baggage", "sentry-org_id=123"),
            ],
        );

        assert_eq!(ctx.incoming_org_id, Some("123".parse().unwrap()));
    }

    #[test]
    fn continue_from_headers_does_not_keep_org_id_without_sentry_trace() {
        let ctx = TransactionContext::continue_from_headers(
            "noop",
            "noop",
            [("baggage", "sentry-org_id=123")],
        );

        assert_eq!(ctx.incoming_org_id, None);
        assert_eq!(ctx.parent_span_id, None);
    }
}
