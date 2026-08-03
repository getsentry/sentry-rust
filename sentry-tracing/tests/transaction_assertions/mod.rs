use sentry::protocol::EnvelopeItem;
use sentry::Envelope;

/// Assert that the given envelopes contain exactly one `Envelope`, containing
/// exactly one `EnvelopeItem`, which is a `Transaction` with the given name.
pub fn assert_transaction(envelopes: Vec<Envelope>, name: &str) {
    let envelope = get_and_assert_only_item(envelopes, "expected exactly one envelope");
    let item = get_and_assert_only_item(envelope.into_items(), "expected exactly one item");

    let EnvelopeItem::Transaction(transaction) = item else {
        panic!("expected a Transaction item, got {item:?}");
    };

    assert_eq!(
        transaction.name.as_deref(),
        Some(name),
        "did not get expected transaction name"
    );
}

/// Gets and asserts that there is exactly one item in the iterator.
fn get_and_assert_only_item<I>(item_iter: I, message: &str) -> I::Item
where
    I: IntoIterator,
{
    let mut iter = item_iter.into_iter();
    let item = iter.next().expect(message);
    assert!(iter.next().is_none(), "{message}");
    item
}
