mod shared;

#[tracing::instrument(fields(initial_field = "value", sentry.name = tracing::field::Empty, sentry.op = tracing::field::Empty))]
fn function_with_dynamic_updates() {
    // Record new sentry attributes dynamically
    tracing::Span::current().record("sentry.name", "updated_span_name");
    tracing::Span::current().record("sentry.op", "updated_operation");
    tracing::error!("event in updated span");
}

#[test]
fn should_update_span_name_and_op_dynamically() {
    let transport = shared::init_sentry(1.0); // Sample all spans.

    function_with_dynamic_updates();

    let data = transport.fetch_and_clear_envelopes();
    assert_eq!(data.len(), 2);

    let event = data.first().expect("should have 1 event");
    let event = match event.items().next().unwrap() {
        sentry::protocol::EnvelopeItem::Event(event) => event,
        unexpected => panic!("Expected event, but got {unexpected:#?}"),
    };

    // Validate transaction trace context shows updated operation
    let trace = match event.contexts.get("trace").expect("to get 'trace' context") {
        sentry::protocol::Context::Trace(trace) => trace,
        unexpected => panic!("Expected trace context but got {unexpected:?}"),
    };
    assert_eq!(trace.op.as_deref().unwrap(), "updated_operation");

    // Confirm transaction has updated values
    let transaction = data.get(1).expect("should have 1 transaction");
    let transaction = match transaction.items().next().unwrap() {
        sentry::protocol::EnvelopeItem::Transaction(transaction) => transaction,
        unexpected => panic!("Expected transaction, but got {unexpected:#?}"),
    };

    // Check that the transaction name was updated
    assert_eq!(transaction.name.as_deref().unwrap(), "updated_span_name");

    // Verify the initial field is still present
    let initial_field = trace
        .data
        .get("initial_field")
        .expect("to have data attribute with name 'initial_field'");
    assert_eq!(initial_field, "value");
}
