#![cfg(feature = "test")]

use std::sync::Arc;

use sentry_core::protocol::{SpanId, TraceId};
use sentry_core::test::TestTransport;
use sentry_core::{Client, ClientOptions, Hub, Transaction};

/// Fixture for starting a transaction from incoming trace headers with a configured client.
struct TraceContinuationScenario {
    incoming_trace_id: TraceId,
    incoming_parent_span_id: SpanId,
    transaction: Transaction,
}

impl TraceContinuationScenario {
    /// Starts a transaction with the given incoming and SDK organization IDs.
    fn run(
        incoming_org_id: Option<&str>,
        sdk_org_id: Option<&str>,
        strict_trace_continuation: bool,
    ) -> Self {
        Self::run_with_options(
            incoming_org_id,
            ClientOptions {
                org_id: sdk_org_id.map(|org_id| org_id.parse().unwrap()),
                strict_trace_continuation,
                traces_sample_rate: 0.0,
                ..Default::default()
            },
        )
    }

    /// Starts a transaction with custom client options, preserving generated incoming IDs.
    ///
    /// `sentry::init` lives in the outer crate, so core tests bind a configured hub directly
    /// and then use the public `start_transaction` entry point.
    fn run_with_options(incoming_org_id: Option<&str>, mut options: ClientOptions) -> Self {
        options
            .dsn
            .get_or_insert_with(|| "https://public@sentry.invalid/1".parse().unwrap());
        options
            .transport
            .get_or_insert_with(|| Arc::new(TestTransport::new()));

        // Generate a random sampled sentry trace header
        let incoming_trace_id = TraceId::default();
        let incoming_parent_span_id = SpanId::default();
        let sentry_trace = format!("{incoming_trace_id}-{incoming_parent_span_id}-1");

        // Construct the transaction context from headers.
        let baggage = incoming_org_id.map(|org_id| format!("sentry-org_id={org_id}"));
        let headers = std::iter::once(("sentry-trace", sentry_trace.as_str()))
            .chain(baggage.as_deref().map(|baggage| ("baggage", baggage)));
        let ctx = sentry_core::TransactionContext::continue_from_headers("noop", "noop", headers);

        // Create the hub with options, then start the transaction.
        let hub = Arc::new(Hub::new(
            Some(Arc::new(Client::with_options(options))),
            Arc::new(Default::default()),
        ));
        let transaction = Hub::run(hub, || sentry_core::start_transaction(ctx));

        Self {
            incoming_trace_id,
            incoming_parent_span_id,
            transaction,
        }
    }

    /// Asserts that the transaction continued the incoming trace and inherited parent sampling.
    fn assert_continued(&self) {
        let context = self.transaction.get_trace_context();
        assert_eq!(context.trace_id, self.incoming_trace_id);
        assert_eq!(context.parent_span_id, Some(self.incoming_parent_span_id));
        assert!(self.transaction.is_sampled());
    }

    /// Asserts that the transaction rejected the incoming trace and parent sampling.
    fn assert_rejected(&self) {
        let context = self.transaction.get_trace_context();
        assert_ne!(context.trace_id, self.incoming_trace_id);
        assert_eq!(context.parent_span_id, None);
        assert!(!self.transaction.is_sampled());
    }
}

#[test]
fn start_transaction_continues_when_no_org_ids_and_not_strict() {
    TraceContinuationScenario::run(None, None, false).assert_continued();
}

#[test]
fn start_transaction_continues_when_no_org_ids_and_strict() {
    TraceContinuationScenario::run(None, None, true).assert_continued();
}

#[test]
fn start_transaction_continues_when_only_incoming_org_id_and_not_strict() {
    TraceContinuationScenario::run(Some("42"), None, false).assert_continued();
}

#[test]
fn start_transaction_rejects_when_only_incoming_org_id_and_strict() {
    TraceContinuationScenario::run(Some("42"), None, true).assert_rejected();
}

#[test]
fn start_transaction_continues_when_only_sdk_org_id_and_not_strict() {
    TraceContinuationScenario::run(None, Some("42"), false).assert_continued();
}

#[test]
fn start_transaction_rejects_when_only_sdk_org_id_and_strict() {
    TraceContinuationScenario::run(None, Some("42"), true).assert_rejected();
}

#[test]
fn start_transaction_continues_when_org_ids_match_and_not_strict() {
    TraceContinuationScenario::run(Some("42"), Some("42"), false).assert_continued();
}

#[test]
fn start_transaction_continues_when_org_ids_match_and_strict() {
    TraceContinuationScenario::run(Some("42"), Some("42"), true).assert_continued();
}

#[test]
fn start_transaction_rejects_when_org_ids_mismatch_and_not_strict() {
    TraceContinuationScenario::run(Some("43"), Some("42"), false).assert_rejected();
}

#[test]
fn start_transaction_rejects_when_org_ids_mismatch_and_strict() {
    TraceContinuationScenario::run(Some("43"), Some("42"), true).assert_rejected();
}

#[test]
fn start_transaction_prefers_explicit_org_id_over_dsn_org_id() {
    TraceContinuationScenario::run_with_options(
        Some("42"),
        ClientOptions {
            dsn: Some("https://public@o43.ingest.sentry.io/1".parse().unwrap()),
            org_id: Some("42".parse().unwrap()),
            strict_trace_continuation: true,
            traces_sample_rate: 0.0,
            ..Default::default()
        },
    )
    .assert_continued();
}
