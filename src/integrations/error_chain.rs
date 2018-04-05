//! Adds support for the error-chain crate.
//!
//! This module is available if the crate is compiled with the `with_error_chain` feature.  It's
//! not enabled by default as error-chain is being replaced by `failure`.
use std::fmt::{Debug, Display};
use error_chain::ChainedError;

use api::protocol::{Event, Exception, Level};
use backtrace_support::{backtrace_to_stacktrace, error_typename};

fn exceptions_from_error_chain<'a, T>(e: &'a T)
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    let mut rv = vec![];
    let error = self.0;

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
pub fn event_from_error_chain<'a, T>(e: &'a T) -> Event
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
pub fn capture_error_chain<'a, T>(e: &'a T) -> Event
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    with_client_and_scope(|client, scope| {
        client.capture_event(event_from_error_chain(e), Some(scope))
    })
}
