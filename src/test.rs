//! This provides testing functionality for building tests.
//!
//! **Feature:** `with_test_support` (*disabled by default*)
//!
//! If the sentry crate has been compiled with the test support feature this
//! module becomes available and provides functionality to create a hub that
//! does not emit to Sentry but instead captures event objects to an internal
//! buffer.
//!
//! # Example usage
//!
//! ```
//! use sentry::{capture_message, Level};
//! use sentry::test::with_captured_events;
//!
//! let events = with_captured_events(|| {
//!     capture_message("Hello World!", Level::Warning);
//! });
//! assert_eq!(events.len(), 1);
//! assert_eq!(events[0].message.as_ref().unwrap(), "Hello World!");
//! ```
use std::sync::Arc;

use client::{Client, ClientOptions};
use hub::Hub;

use protocol::Event;
use Dsn;

lazy_static! {
    static ref TEST_DSN: Dsn = "https://public@sentry.invalid/1".parse().unwrap();
}

/// Creates a testable hub.
///
/// A test hub will never send to an upstream Sentry service but instead
/// uses an internal transport that just locally captures events.  Utilities
/// from this module can be used to interact with it.  Testable hubs
/// internally use a different client that does not send events and they
/// are always wrapped in an `Arc`.  This uses a hardcoded internal test
/// only DSN.
///
/// To access test specific functionality on hubs you need to bring the
/// `TestableHubExt` into the scope.
///
/// # Example
///
/// ```
/// use sentry::test::{create_testable_hub, TestableHubExt};
/// let hub = create_testable_hub(Default::default());
/// let events = hub.run_and_capture_events(|| {
///     // in here `sentry::Hub::current()` returns our testable hub.
///     // any event captured will go to the hub.
/// });
/// ```
pub fn create_testable_hub(options: ClientOptions) -> Arc<Hub> {
    Arc::new(Hub::new(
        Some(Arc::new(Client::testable(TEST_DSN.clone(), options))),
        Arc::new(Default::default()),
    ))
}

/// Extensions for working with testable hubs.
///
/// Because a testable hub by itself cannot be told from a non testable hub
/// this trait needs to be used to access extra functionality on a testable
/// hub such as fetching the buffered events.
///
/// For convenience reasons testable hubs are always wrapped in `Arc` wrappers
/// so that they can directly be bound to the current thread.  This means
/// this trait is only implemented for `Arc<Hub>` and not for `Hub` directly.
pub trait TestableHubExt {
    /// Checks if the hub is a testable hub.
    fn is_testable(&self) -> bool;

    /// Fetches events from a testable hub.
    ///
    /// This removes all the events from the internal buffer and empties it.
    fn fetch_events(&self) -> Vec<Event<'static>>;

    /// Runs code with the bound hub and fetches the events.
    fn run_and_capture_events<F: FnOnce()>(&self, f: F) -> Vec<Event<'static>>;
}

impl TestableHubExt for Arc<Hub> {
    fn is_testable(&self) -> bool {
        if let Some(client) = self.client() {
            client.dsn().is_some() && client.transport().is_test()
        } else {
            false
        }
    }

    fn fetch_events(&self) -> Vec<Event<'static>> {
        self.client()
            .expect("need a hub with client")
            .transport()
            .fetch_and_clear_events()
    }

    fn run_and_capture_events<F: FnOnce()>(&self, f: F) -> Vec<Event<'static>> {
        Hub::run_bound(self.clone(), f);
        self.fetch_events()
    }
}

/// Runs some code with the default test hub and returns the captured events.
///
/// This is a shortcut for creating a testable hub with default options and
/// to call `run_and_capture_events` on it.
pub fn with_captured_events<F: FnOnce()>(f: F) -> Vec<Event<'static>> {
    with_captured_events_options(f, Default::default())
}

/// Runs some code with the default test hub with the given optoins and
/// returns the captured events.
///
/// This is a shortcut for creating a testable hub with the supplied options
/// and to call `run_and_capture_events` on it.
pub fn with_captured_events_options<F: FnOnce()>(
    f: F,
    options: ClientOptions,
) -> Vec<Event<'static>> {
    create_testable_hub(options).run_and_capture_events(f)
}
