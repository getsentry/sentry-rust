//! Adds support for capturing Sentry errors from `anyhow::Error`.
//!
//! # Example
//!
//! ```no_run
//! # fn function_that_might_fail() -> anyhow::Result<()> { Ok(()) }
//! use sentry_anyhow::capture_anyhow;
//! # fn test() -> anyhow::Result<()> {
//! let result = match function_that_might_fail() {
//!     Ok(result) => result,
//!     Err(err) => {
//!         capture_anyhow(&err);
//!         return Err(err);
//!     }
//! };
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]

use std::error::Error;
use std::fmt;

use sentry_core::types::Uuid;
use sentry_core::Hub;

/// Captures an `anyhow::Error`.
///
/// See [module level documentation](index.html) for more information.
pub fn capture_anyhow(e: &anyhow::Error) -> Uuid {
    Hub::with_active(|hub| hub.capture_anyhow(e))
}

/// Hub extension methods for working with `anyhow`.
pub trait AnyhowHubExt {
    /// Captures an `anyhow::Error` on a specific hub.
    fn capture_anyhow(&self, e: &anyhow::Error) -> Uuid;
}

impl AnyhowHubExt for Hub {
    fn capture_anyhow(&self, e: &anyhow::Error) -> Uuid {
        self.capture_error(&AnyhowError(e))
    }
}

// `anyhow::Error` itself does not impl `std::error::Error`, because it would
// be incoherent. This can be worked around by wrapping it in a newtype
// which impls `std::error::Error`.
// Code adopted from: https://github.com/dtolnay/anyhow/issues/63#issuecomment-590983511
struct AnyhowError<'a>(&'a anyhow::Error);

impl fmt::Debug for AnyhowError<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl fmt::Display for AnyhowError<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl Error for AnyhowError<'_> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}
