#![cfg(feature = "test")]

use std::sync::Arc;

use sentry_core::protocol::client_report::Reason;
use sentry_core::protocol::{EnvelopeItem, Event};
use sentry_core::test::TestTransport;
use sentry_core::{Client, ClientOptions, Envelope, Hub, Integration, Scope, TransactionContext};

struct DroppingIntegration;

impl Integration for DroppingIntegration {
    fn process_event(
        &self,
        _event: Event<'static>,
        _options: &ClientOptions,
    ) -> Option<Event<'static>> {
        None
    }
}

fn client_with_options(transport: Arc<TestTransport>, options: ClientOptions) -> Client {
    Client::with_options(
        options
            .dsn("https://public@sentry.invalid/1")
            .transport(transport),
    )
}

fn assert_client_report(envelope: &Envelope, expected: serde_json::Value) {
    let client_report = envelope
        .items()
        .find_map(|item| match item {
            EnvelopeItem::ClientReport(report) => Some(report),
            _ => None,
        })
        .expect("envelope should contain a client report");

    let value = serde_json::to_value(client_report).unwrap();
    assert_eq!(value["discarded_events"], expected);
}

fn assert_drop_records_client_report<F>(options: ClientOptions, capture: F, reason: Reason)
where
    F: FnOnce(&Client),
{
    let transport = TestTransport::new();
    let client = client_with_options(transport.clone(), options);

    capture(&client);
    client.send_envelope(Envelope::new());

    let envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(envelopes.len(), 1);
    assert_client_report(
        &envelopes[0],
        serde_json::json!([{ "category": "error", "reason": reason, "quantity": 1 }]),
    );
}

#[test]
fn client_report_records_scope_event_processor_drop() {
    let mut scope = Scope::default();
    scope.add_event_processor(|_| None);

    assert_drop_records_client_report(
        ClientOptions::default(),
        |client| {
            client.capture_event(Event::default(), Some(&scope));
        },
        Reason::EventProcessor,
    );
}

#[test]
fn client_report_records_integration_event_processor_drop() {
    let options = ClientOptions::new().add_integration(DroppingIntegration);

    assert_drop_records_client_report(
        options,
        |client| {
            client.capture_event(Event::default(), None);
        },
        Reason::EventProcessor,
    );
}

#[test]
fn client_report_records_before_send_drop() {
    let options = ClientOptions::new().before_send(|_| None);

    assert_drop_records_client_report(
        options,
        |client| {
            client.capture_event(Event::default(), None);
        },
        Reason::BeforeSend,
    );
}

#[test]
fn client_report_records_sample_rate_drop() {
    let options = ClientOptions::new().sample_rate(0.0);

    assert_drop_records_client_report(
        options,
        |client| {
            client.capture_event(Event::default(), None);
        },
        Reason::SampleRate,
    );
}

#[test]
fn client_report_records_unsampled_transaction_and_spans() {
    let transport = TestTransport::new();
    let client = Arc::new(client_with_options(
        transport.clone(),
        ClientOptions::new().traces_sample_rate(0.0),
    ));

    Hub::run(
        Arc::new(Hub::new(Some(client.clone()), Arc::new(Default::default()))),
        || {
            let transaction = sentry_core::start_transaction(TransactionContext::new("tx", "op"));
            transaction.start_child("child", "one").finish();
            transaction.start_child("child", "two").finish();
            transaction.finish();
        },
    );
    client.send_envelope(Envelope::new());

    let envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(envelopes.len(), 1);
    assert_client_report(
        &envelopes[0],
        serde_json::json!([
            { "category": "transaction", "reason": "sample_rate", "quantity": 1 },
            { "category": "span", "reason": "sample_rate", "quantity": 3 },
        ]),
    );
}

#[test]
fn client_report_records_transaction_span_cap_drop() {
    // Keep in sync with `MAX_SPANS` in `sentry-core/src/performance.rs`.
    const MAX_SPANS: usize = 1_000;

    let transport = TestTransport::new();
    let client = Arc::new(client_with_options(
        transport.clone(),
        ClientOptions::new().traces_sample_rate(1.0),
    ));

    Hub::run(
        Arc::new(Hub::new(Some(client.clone()), Arc::new(Default::default()))),
        || {
            let transaction = sentry_core::start_transaction(TransactionContext::new("tx", "op"));
            for _ in 0..=MAX_SPANS {
                transaction.start_child("child", "kept").finish();
            }
            transaction.start_child("child", "dropped").finish();
        },
    );
    client.send_envelope(Envelope::new());

    let envelopes = transport.fetch_and_clear_envelopes();
    assert_eq!(envelopes.len(), 1);
    assert_client_report(
        &envelopes[0],
        serde_json::json!([{ "category": "span", "reason": "buffer_overflow", "quantity": 1 }]),
    );
}
