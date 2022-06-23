//! Adds support for capturing Sentry errors from [`anyhow::Error`].
//!
//! This integration adds a new event *source*, which allows you to create events directly
//! from an [`anyhow::Error`] struct.  As it is only an event source it only needs to be
//! enabled using the `anyhow` cargo feature, it does not need to be enabled in the call to
//! [`sentry::init`](https://docs.rs/sentry/*/sentry/fn.init.html).
//!
//! This integration does not need to be installed, instead it provides an extra function to
//! capture [`anyhow::Error`], optionally exposing it as a method on the
//! [`sentry::Hub`](https://docs.rs/sentry/*/sentry/struct.Hub.html) using the
//! [`AnyhowHubExt`] trait.
//!
//! Like a plain [`std::error::Error`] being captured, [`anyhow::Error`] is captured with a
//! chain of all error sources, if present.  See
//! [`sentry::capture_error`](https://docs.rs/sentry/*/sentry/fn.capture_error.html) for
//! details of this.
//!
//! # Example
//!
//! ```no_run
//! use sentry_anyhow::capture_anyhow;
//!
//! fn function_that_might_fail() -> anyhow::Result<()> {
//!     Err(anyhow::anyhow!("some kind of error"))
//! }
//!
//! if let Err(err) = function_that_might_fail() {
//!     capture_anyhow(&err);
//! }
//! ```
//!
//! # Features
//!
//! The `backtrace` feature will enable the corresponding feature in anyhow and allow you to
//! capture backtraces with your events.  It is enabled by default.
//!
//! [`anyhow::Error`]: https://docs.rs/anyhow/*/anyhow/struct.Error.html

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

use sentry_core::protocol::Event;
use sentry_core::types::Uuid;
use sentry_core::Hub;

/// Captures an [`anyhow::Error`].
///
/// This will capture an anyhow error as a sentry event if a
/// [`sentry::Client`](../../struct.Client.html) is initialised, otherwise it will be a
/// no-op.  The event is dispatched to the thread-local hub, with semantics as described in
/// [`Hub::current`].
///
/// See [module level documentation](index.html) for more information.
///
/// [`anyhow::Error`]: https://docs.rs/anyhow/*/anyhow/struct.Error.html
pub fn capture_anyhow(e: &anyhow::Error) -> Uuid {
    Hub::with_active(|hub| hub.capture_anyhow(e))
}

/// Helper function to create an event from a `anyhow::Error`.
pub fn event_from_error(err: &anyhow::Error) -> Event<'static> {
    let dyn_err: &dyn std::error::Error = err.as_ref();

    // It's not mutated for not(feature = "backtrace")
    #[allow(unused_mut)]
    let mut event = sentry_core::event_from_error(dyn_err);

    #[cfg(feature = "backtrace")]
    {
        // exception records are sorted in reverse
        if let Some(exc) = event.exception.iter_mut().last() {
            let backtrace = err.backtrace();
            exc.stacktrace = sentry_backtrace::parse_stacktrace(&format!("{:#}", backtrace));
        }
    }

    event
}

/// Hub extension methods for working with [`anyhow`].
pub trait AnyhowHubExt {
    /// Captures an [`anyhow::Error`] on a specific hub.
    fn capture_anyhow(&self, e: &anyhow::Error) -> Uuid;
}

impl AnyhowHubExt for Hub {
    fn capture_anyhow(&self, anyhow_error: &anyhow::Error) -> Uuid {
        let event = event_from_error(anyhow_error);
        self.capture_event(event)
    }
}

#[cfg(all(feature = "backtrace", test))]
mod tests {
    use super::*;

    #[test]
    fn test_event_from_error_with_backtrace() {
        std::env::set_var("RUST_BACKTRACE", "1");

        let event = event_from_error(&anyhow::anyhow!("Oh jeez"));

        let stacktrace = event.exception[0].stacktrace.as_ref().unwrap();
        let found_test_fn = stacktrace
            .frames
            .iter()
            .find(|frame| match &frame.function {
                Some(f) => f.contains("test_event_from_error_with_backtrace"),
                None => false,
            });

        assert!(found_test_fn.is_some());
    }

    #[test]
    fn test_capture_anyhow_uses_event_from_error_helper() {
        std::env::set_var("RUST_BACKTRACE", "1");

        let err = &anyhow::anyhow!("Oh jeez");

        let event = event_from_error(err);
        let events = sentry::test::with_captured_events(|| {
            capture_anyhow(err);
        });

        assert_eq!(event.exception, events[0].exception);
    }
}
