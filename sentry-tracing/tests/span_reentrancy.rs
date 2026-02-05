use sentry::protocol::EnvelopeItem;

mod shared;

/// Ensures re-entering the same span does not corrupt the current tracing state,
/// so subsequent spans are still recorded under a single transaction.
#[test]
fn reentering_span_preserves_parent() {
    let transport = shared::init_sentry(1.0);

    {
        // Create a span and enter it, then re-enter the same span to simulate
        // common async polling behavior where a span can be entered multiple times.
        let span_a = tracing::info_span!("a");
        let _enter_a = span_a.enter();
        {
            let _reenter_a = span_a.enter();
        }

        // Create another span while the original span is still entered to ensure
        // it is recorded on the same transaction rather than starting a new one.
        let span_b = tracing::info_span!("b");
        {
            let _enter_b = span_b.enter();
        }
    }

    let transactions: Vec<_> = transport
        .fetch_and_clear_envelopes()
        .into_iter()
        .flat_map(|envelope| envelope.into_items())
        .filter_map(|item| match item {
            EnvelopeItem::Transaction(transaction) => Some(transaction),
            _ => None,
        })
        .collect();

    assert_eq!(
        transactions.len(),
        1,
        "expected a single transaction when reentering a span"
    );

    let transaction = &transactions[0];
    assert_eq!(transaction.name.as_deref(), Some("a"));

    let trace = match transaction
        .contexts
        .get("trace")
        .expect("transaction should include trace context")
    {
        sentry::protocol::Context::Trace(trace) => trace,
        unexpected => panic!("expected trace context but got {unexpected:?}"),
    };

    let b_span = transaction
        .spans
        .iter()
        .find(|span| span.description.as_deref() == Some("b"))
        .expect("expected span \"b\" to be recorded in the transaction");

    assert_eq!(b_span.parent_span_id, Some(trace.span_id));
    assert!(
        !transaction
            .spans
            .iter()
            .any(|span| span.description.as_deref() == Some("a")),
        "expected the transaction root span not to be duplicated in span list"
    );
}
