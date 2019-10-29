//! Adds support for the anyhow crate.
//!
//! **Feature:** `with_anyhow` (disabled by default)
//!
//! This does not support capturing backtraces on nightly.
//!
//! # Example
//!
//! ```no_run
//! # extern crate sentry;
//! # extern crate anyhow;
//! # fn function_that_might_fail() -> Result<(), anyhow::Error> { Ok(()) }
//! use sentry::integrations::anyhow::capture_error;
//! # fn test() -> Result<(), anyhow::Error> {
//! let result = match function_that_might_fail() {
//!     Ok(result) => result,
//!     Err(err) => {
//!         capture_error(&err);
//!         return Err(err);
//!     }
//! };
//! # Ok(()) }
//! # fn main() { test().unwrap() }
//! ```
//!
use anyhow::Error;

use crate::hub::Hub;
use crate::internals::Uuid;
use crate::protocol::{Event, Exception, Level};

/// Helper function to create an event from a `anyhow::Error`.
pub fn event_from_error(err: &anyhow::Error) -> Event<'static> {
    let mut exceptions = vec![];

    for cause in err.chain() {
        exceptions.push(Exception {
            ty: "Error".to_owned(),
            module: None,
            value: Some(cause.to_string()),
            stacktrace: None,
            ..Default::default()
        });
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: Level::Error,
        ..Default::default()
    }
}

/// Captures a boxed failure (`anyhow::Error`).
///
/// This dispatches to the current hub.
pub fn capture_error(err: &Error) -> Uuid {
    Hub::with_active(|hub| hub.capture_error(err))
}

/// Hub extension methods for working with failure.
pub trait AnyhowHubExt {
    /// Captures a boxed failure (`anyhow::Error`).
    fn capture_error(&self, err: &Error) -> Uuid;
}

impl AnyhowHubExt for Hub {
    fn capture_error(&self, err: &Error) -> Uuid {
        self.capture_event(event_from_error(err))
    }
}

/// Extension trait providing methods to unwrap a result, preserving backtraces from the
/// underlying error in the event of a panic.
pub trait AnyhowResultExt {
    /// Type of the success case
    type Value;
    /// Unwraps the result, panicking if it contains an error. Any backtrace attached to the
    /// error will be preserved with the panic.
    fn fallible_unwrap(self) -> Self::Value;
}

impl<T, E> AnyhowResultExt for Result<T, E>
where
    E: Into<Error>,
{
    type Value = T;
    fn fallible_unwrap(self) -> Self::Value {
        match self {
            Ok(v) => v,
            Err(e) => {
                let e: Error = e.into();
                panic!(e)
            }
        }
    }
}
