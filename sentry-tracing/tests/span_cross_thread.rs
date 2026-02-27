mod shared;

use sentry::protocol::Context;
use std::thread;
use std::time::Duration;

#[test]
fn cross_thread_span_entries_share_transaction() {
    let transport = shared::init_sentry(1.0);

    let span = tracing::info_span!("foo");
    let span2 = span.clone();

    let handle1 = thread::spawn(move || {
        let _guard = span.enter();
        let _bar_span = tracing::info_span!("bar").entered();
        thread::sleep(Duration::from_millis(100));
    });

    let handle2 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        let _guard = span2.enter();
        let _baz_span = tracing::info_span!("baz").entered();
        thread::sleep(Duration::from_millis(50));
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    let data = transport.fetch_and_clear_envelopes();
    let transactions: Vec<_> = data
        .into_iter()
        .flat_map(|envelope| {
            envelope
                .items()
                .filter_map(|item| match item {
                    sentry::protocol::EnvelopeItem::Transaction(transaction) => {
                        Some(transaction.clone())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(
        transactions.len(),
        1,
        "expected a single transaction for cross-thread span entries"
    );

    let transaction = &transactions[0];
    assert_eq!(transaction.name.as_deref(), Some("foo"));

    let trace = match transaction
        .contexts
        .get("trace")
        .expect("transaction should include trace context")
    {
        Context::Trace(trace) => trace,
        unexpected => panic!("expected trace context but got {unexpected:?}"),
    };

    let bar_span = transaction
        .spans
        .iter()
        .find(|span| span.description.as_deref() == Some("bar"))
        .expect("expected span \"bar\" to be recorded in the transaction");
    let baz_span = transaction
        .spans
        .iter()
        .find(|span| span.description.as_deref() == Some("baz"))
        .expect("expected span \"baz\" to be recorded in the transaction");

    assert_eq!(bar_span.parent_span_id, Some(trace.span_id));
    assert_eq!(baz_span.parent_span_id, Some(trace.span_id));
}
