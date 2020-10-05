//! Adds support for capturing Sentry errors from `anyhow::Error`.
//!
//! # Example
//!
//! ```no_run
//! use sentry_anyhow::{capture_anyhow, AnyhowIntegration};
//!
//! fn function_that_might_fail() -> anyhow::Result<()> {
//!     Err(anyhow::anyhow!("some kind of error"))
//! }
//!
//! let _sentry =
//!     sentry::init(sentry::ClientOptions::new().add_integration(AnyhowIntegration));
//!
//! if let Err(err) = function_that_might_fail() {
//!     capture_anyhow(&err);
//! }
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

use sentry_core::types::Uuid;
use sentry_core::{ClientOptions, Hub, Integration};

/// The Sentry anyhow Integration.
#[derive(Debug, Default)]
pub struct AnyhowIntegration;

impl AnyhowIntegration {
    /// Creates a new anyhow Integration.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Integration for AnyhowIntegration {
    fn name(&self) -> &'static str {
        "anyhow"
    }

    fn setup(&self, cfg: &mut ClientOptions) {
        cfg.in_app_exclude.push("anyhow::");
    }
}

/// Captures an `anyhow::Error`.
///
/// See [module level documentation](index.html) for more information.
pub fn capture_anyhow(e: &anyhow::Error) -> Uuid {
    Hub::with_active(|hub| hub.capture_anyhow(e))
}

/// Hub extension methods for working with `anyhow`.
pub trait AnyhowHubExt {
    /// Captures an [`anyhow::Error`] on a specific hub.
    fn capture_anyhow(&self, e: &anyhow::Error) -> Uuid;
}

impl AnyhowHubExt for Hub {
    fn capture_anyhow(&self, e: &anyhow::Error) -> Uuid {
        let e: &dyn std::error::Error = e.as_ref();
        self.capture_error(e)
    }
}
