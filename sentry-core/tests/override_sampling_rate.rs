#![cfg(feature = "test")]

use sentry_core::protocol::{EnvelopeItem, Event};
use sentry_core::test::TestTransport;
use sentry_core::{Client, ClientOptions};

fn captured_event_count(options: ClientOptions) -> usize {
    let transport = TestTransport::new();
    let client = Client::with_options(
        options
            .dsn("https://public@sentry.invalid/1")
            .transport(transport.clone()),
    );

    client.capture_event(Event::default(), None);

    transport
        .fetch_and_clear_envelopes()
        .iter()
        .flat_map(|envelope| envelope.items())
        .filter(|item| matches!(item, EnvelopeItem::Event(_)))
        .count()
}

#[test]
fn override_zero_drops_event() {
    let options = ClientOptions::new().override_sampling_rate(|_| Some(0.0));
    assert_eq!(captured_event_count(options), 0);
}

#[test]
fn override_takes_precedence_over_sample_rate() {
    let options = ClientOptions::new()
        .sample_rate(0.0)
        .override_sampling_rate(|_| Some(1.0));
    assert_eq!(captured_event_count(options), 1);
}

#[test]
fn override_none_falls_back_to_sample_rate() {
    let options = ClientOptions::new()
        .sample_rate(0.0)
        .override_sampling_rate(|_| None);
    assert_eq!(captured_event_count(options), 0);
}

#[test]
fn override_nan_falls_back_to_sample_rate() {
    // NaN would fail every sampling comparison and silently drop the event,
    // so it must be ignored in favor of the configured strategy (1.0 here).
    let options = ClientOptions::new().override_sampling_rate(|_| Some(f32::NAN));
    assert_eq!(captured_event_count(options), 1);
}
