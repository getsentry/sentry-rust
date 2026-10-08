#![expect(missing_docs, reason = "predates lint enforcement")]

mod shared;

use sentry_tracing::SpanLayer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;

#[tracing::instrument(fields(
    some = "value",
    sentry.name = "updated name",
    sentry.op = "updated op",
))]
fn test_fun_record_on_creation() {}

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
    let _subscriber = tracing_subscriber::registry()
        .with(SpanLayer::new().with_filter(LevelFilter::INFO))
        .set_default();

    for f in [test_fun_record_on_creation, test_fun_record_later] {
        f();

        let data = transport.fetch_and_clear_envelopes();
        assert_eq!(data.len(), 1);

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
