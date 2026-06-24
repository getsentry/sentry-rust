#![cfg(feature = "test")]

use std::sync::Arc;

use sentry_core::protocol::client_report::Reason;
use sentry_core::protocol::{EnvelopeItem, Event};
use sentry_core::test::TestTransport;
use sentry_core::{Client, ClientOptions, Envelope, Integration, Scope};

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
    Client::with_options(ClientOptions {
        dsn: Some("https://public@sentry.invalid/1".parse().unwrap()),
        transport: Some(Arc::new(transport)),
        ..options
    })
}

fn assert_client_report(envelope: &Envelope, reason: Reason) {
    let client_report = envelope
        .items()
        .find_map(|item| match item {
            EnvelopeItem::ClientReport(report) => Some(report),
            _ => None,
        })
        .expect("envelope should contain a client report");

    let value = serde_json::to_value(client_report).unwrap();
    assert_eq!(
        value["discarded_events"],
        serde_json::json!([{ "category": "error", "reason": reason, "quantity": 1 }])
    );
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
    assert_client_report(&envelopes[0], reason);
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
    let options = ClientOptions {
        before_send: Some(Arc::new(|_| None)),
        ..Default::default()
    };

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
    let options = ClientOptions {
        sample_rate: 0.0,
        ..Default::default()
    };

    assert_drop_records_client_report(
        options,
        |client| {
            client.capture_event(Event::default(), None);
        },
        Reason::SampleRate,
    );
}
