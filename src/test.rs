//! This provides testing functionality for building tests.
//!
//! **Feature:** `with_test_support` (*disabled by default*)
use std::sync::Arc;

use client::{Client, ClientOptions};
use hub::Hub;

use protocol::Event;
use Dsn;

lazy_static! {
    static ref TEST_DSN: Dsn = "https://public@sentry.invalid/1".parse().unwrap();
}

/// Creates a test hub.
///
/// A test hub will never send to an upstream Sentry service but instead
/// uses an internal transport that just locally captures events.  Utilities
/// from this module can be used to interact with it.
pub fn create_testable_hub(options: ClientOptions) -> Hub {
    Hub::new(
        Some(Arc::new(Client::testable(TEST_DSN.clone(), options))),
        Arc::new(Default::default()),
    )
}

/// Fetches all events from a testable hub.
pub fn fetch_events(hub: &Hub) -> Vec<Event<'static>> {
    hub.client()
        .expect("need a hub with client")
        .transport()
        .fetch_and_clear_events()
}

/// Runs some code in the context of the default test hub and returns the
/// captured events.
pub fn with_captured_events<F: FnOnce()>(f: F) -> Vec<Event<'static>> {
    with_captured_events_options(f, Default::default())
}

/// Runs some code in the context of the default test hub and returns the
/// captured events.
pub fn with_captured_events_options<F: FnOnce()>(
    f: F,
    options: ClientOptions,
) -> Vec<Event<'static>> {
    let hub = Arc::new(create_testable_hub(options));
    Hub::run_bound(hub.clone(), f);
    fetch_events(&hub)
}
