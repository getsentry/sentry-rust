//! Adds support for capturing Sentry errors from [`anyhow::Error`].
//!
//! This integration adds a new event *source*, which allows you to create events directly
//! from an [`anyhow::Error`] struct.  As it is only an event source it only needs to be
//! enabled using the `anyhow` cargo feature, it does not need to be enabled in the call to
//! [`sentry::init`](../../fn.init.html).
//!
//! This integration does not need to be installed, instead it provides an extra function to
//! capture [`anyhow::Error`], optionally exposing it as a method on the
//! [`sentry::Hub`](../../struct.Hub.html) using the [`AnyhowHubExt`] trait.
//!
//! Like a plain [`std::error::Error`] being captured, [`anyhow::Error`] is captured with a
//! chain of all error sources, if present.  See
//! [`sentry::capture_error`](../../fn.capture_error.html) for details of this.
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
//! [`anyhow::Error`]: https://docs.rs/anyhow/*/anyhow/struct.Error.html

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

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

/// Hub extension methods for working with [`anyhow`].
///
/// [`anyhow`]: https://docs.rs/anyhow
pub trait AnyhowHubExt {
    /// Captures an [`anyhow::Error`] on a specific hub.
    ///
    /// [`anyhow::Error`]: https://docs.rs/anyhow/*/anyhow/struct.Error.html
    fn capture_anyhow(&self, e: &anyhow::Error) -> Uuid;
}

impl AnyhowHubExt for Hub {
    fn capture_anyhow(&self, e: &anyhow::Error) -> Uuid {
        let e: &dyn std::error::Error = e.as_ref();
        self.capture_error(e)
    }
}
