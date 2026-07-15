use sentry::protocol::{EnvelopeItem, Transaction};
use sentry::Envelope;

/// Assert that the given envelopes contain exactly one `Envelope`, containing
/// exactly one `EnvelopeItem`, which is a `Transaction` with the given name.
pub fn assert_transaction(envelopes: Vec<Envelope>, name: &str) {
    let envelope = get_and_assert_only_item(envelopes, "expected exactly one envelope");
    let item = get_and_assert_only_item(envelope.into_items(), "expected exactly one item");

    assert!(
        matches!(
            item,
            EnvelopeItem::Transaction(Transaction {
                name: Some(expected_name),
                ..
            }) if expected_name == name
        ),
        "expected a Transaction item with name {name:?}"
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
