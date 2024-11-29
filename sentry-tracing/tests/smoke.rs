mod shared;

#[tracing::instrument(fields(tags.tag = "key", not_tag = "value"))]
fn function_with_tags(value: i32) {
    tracing::error!(value, "event");
}

#[test]
fn should_instrument_function_with_event() {
    let transport = shared::init_sentry(1.0); // Sample all spans.

    function_with_tags(1);

    let data = transport.fetch_and_clear_envelopes();
    assert_eq!(data.len(), 2);
    let event = data.first().expect("should have 1 event");
    let event = match event.items().next().unwrap() {
        sentry::protocol::EnvelopeItem::Event(event) => event,
        unexpected => panic!("Expected event, but got {:#?}", unexpected),
    };

    //Validate transaction is created
    let trace = match event.contexts.get("trace").expect("to get 'trace' context") {
        sentry::protocol::Context::Trace(trace) => trace,
        unexpected => panic!("Expected trace context but got {:?}", unexpected),
    };
    assert_eq!(trace.op.as_deref().unwrap(), "function_with_tags");

    //Confirm transaction values
    let transaction = data.get(1).expect("should have 1 transaction");
    let transaction = match transaction.items().next().unwrap() {
        sentry::protocol::EnvelopeItem::Transaction(transaction) => transaction,
        unexpected => panic!("Expected transaction, but got {:#?}", unexpected),
    };
    assert_eq!(transaction.tags.len(), 1);
    assert_eq!(transaction.extra.len(), 2);

    let tag = transaction
        .tags
        .get("tag")
        .expect("to have tag with name 'tag'");
    assert_eq!(tag, "key");
    let not_tag = transaction
        .extra
        .get("not_tag")
        .expect("to have extra with name 'not_tag'");
    assert_eq!(not_tag, "value");
    let value = transaction
        .extra
        .get("value")
        .expect("to have extra with name 'value'");
    assert_eq!(value, 1);
}
