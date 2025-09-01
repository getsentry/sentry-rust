mod shared;

#[tracing::instrument(fields(
    some = "value",
    sentry.name = tracing::field::Empty,
    sentry.op = tracing::field::Empty,
))]
fn test_fun_record_on_creation() {
    tracing::Span::current().record("sentry.name", "updated name");
    tracing::Span::current().record("sentry.op", "updated op");
}

#[tracing::instrument(fields(
    some = "value",
    sentry.name = tracing::field::Empty,
    sentry.op = tracing::field::Empty,
))]
fn test_fun_record_later() {
    tracing::Span::current().record("sentry.name", "updated name");
    tracing::Span::current().record("sentry.op", "updated op");
}

#[test]
fn should_update_sentry_op_and_name_based_on_fields() {
    let transport = shared::init_sentry(1.0);

    for f in [test_fun_record_on_creation, test_fun_record_later] {
        f();

        let data = transport.fetch_and_clear_envelopes();
        assert_eq!(data.len(), 1);
        // Confirm transaction has updated values
        let transaction = data.first().expect("should have 1 transaction");
        let transaction = match transaction.items().next().unwrap() {
            sentry::protocol::EnvelopeItem::Transaction(transaction) => transaction,
            unexpected => panic!("Expected transaction, but got {unexpected:#?}"),
        };

        assert_eq!(transaction.name.as_deref().unwrap(), "updated name");
        let ctx = transaction.contexts.get("trace");
        match ctx {
            Some(sentry::protocol::Context::Trace(trace_ctx)) => {
                assert_eq!(trace_ctx.op, Some("updated op".to_owned()))
            }
            _ => panic!("expected trace context"),
        }
    }
}
