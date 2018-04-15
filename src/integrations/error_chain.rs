//! Adds support for the error-chain crate.
//!
//! **Feature:** `with_error_chain` (disabled by default)
//!
//! Errors created by the `error-chain` crate can be logged with the
//! `error_chain` integration.
//!
//! # Example
//!
//! ```no_run
//! # extern crate sentry;
//! # #[macro_use] extern crate error_chain;
//! # error_chain! {}
//! use sentry::integrations::error_chain::capture_error_chain;
//! # fn function_that_might_fail() -> Result<()> { Ok(()) }
//! # fn test() -> Result<()> {
//! let result = match function_that_might_fail() {
//!     Ok(result) => result,
//!     Err(err) => {
//!         capture_error_chain(&err);
//!         return Err(err);
//!     }
//! };
//! # Ok(()) }
//! # fn main() { test().unwrap() }
//! ```
use std::fmt::{Debug, Display};

use uuid::Uuid;
use error_chain::ChainedError;

use api::protocol::{Event, Exception, Level};
use scope::with_client_and_scope;
use backtrace_support::{backtrace_to_stacktrace, error_typename};

fn exceptions_from_error_chain<'a, T>(error: &'a T) -> Vec<Exception>
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    let mut rv = vec![];

    rv.push(Exception {
        ty: error_typename(error.kind()),
        value: Some(error.kind().to_string()),
        stacktrace: error.backtrace().and_then(backtrace_to_stacktrace),
        ..Default::default()
    });

    for error in error.iter().skip(1) {
        rv.push(Exception {
            ty: error_typename(error),
            value: Some(error.to_string()),
            ..Default::default()
        })
    }

    rv
}

/// Creates an event from an error chain.
pub fn event_from_error_chain<'a, T>(e: &'a T) -> Event<'static>
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    Event {
        exceptions: exceptions_from_error_chain(e),
        level: Level::Error,
        ..Default::default()
    }
}

/// Captures an error chain.
pub fn capture_error_chain<'a, T>(e: &'a T) -> Uuid
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    with_client_and_scope(|client, scope| {
        client.capture_event(event_from_error_chain(e), Some(scope))
    })
}
