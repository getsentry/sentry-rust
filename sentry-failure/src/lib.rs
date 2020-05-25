//! Adds support for capturing Sentry errors from `failure` types.
//!
//! Failure errors and `Fail` objects can be logged with the failure integration.
//! This works really well if you use the `failure::Error` type or if you have
//! `failure::Fail` objects that use the failure context internally to gain a
//! backtrace.
//!
//! # Example
//!
//! ```no_run
//! # use sentry_core as sentry;
//! # fn function_that_might_fail() -> Result<(), failure::Error> { Ok(()) }
//! use sentry_failure::capture_error;
//! # fn test() -> Result<(), failure::Error> {
//! let result = match function_that_might_fail() {
//!     Ok(result) => result,
//!     Err(err) => {
//!         capture_error(&err);
//!         return Err(err);
//!     }
//! };
//! # Ok(()) }
//! ```
//!
//! To capture fails and not errors use `capture_fail`.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![warn(missing_doc_code_examples)]

use std::panic::PanicInfo;

use failure::{Error, Fail};
use sentry_backtrace::parse_stacktrace;
use sentry_core::parse_type_from_debug;
use sentry_core::protocol::{Event, Exception, Level};
use sentry_core::types::Uuid;
use sentry_core::{ClientOptions, Hub, Integration};

/// The Sentry Failure Integration.
#[derive(Default)]
pub struct FailureIntegration;

impl FailureIntegration {
    /// Creates a new Failure Integration.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Integration for FailureIntegration {
    fn name(&self) -> &'static str {
        "failure"
    }

    fn setup(&self, cfg: &mut ClientOptions) {
        cfg.in_app_exclude.push("failure::");
        cfg.extra_border_frames.extend_from_slice(&[
            "failure::error_message::err_msg",
            "failure::backtrace::Backtrace::new",
            "failure::backtrace::internal::InternalBacktrace::new",
            "failure::Fail::context",
        ]);
    }
}

/// Extracts a Sentry `Event` from a `PanicInfo`.
///
/// Creates a new Sentry `Event` when the panic has a `failure::Error` payload.
/// This is for use with the `sentry-panic` integration, and is enabled by
/// default in `sentry`.
///
/// # Examples
///
/// ```
/// let panic_integration =
///     sentry_panic::PanicIntegration::new().add_extractor(sentry_failure::panic_extractor);
/// ```
pub fn panic_extractor(info: &PanicInfo<'_>) -> Option<Event<'static>> {
    let error = info.payload().downcast_ref::<Error>()?;
    Some(Event {
        level: Level::Fatal,
        ..event_from_error(error)
    })
}

/// This converts a single fail instance into an exception.
///
/// This is typically not very useful as the `event_from_error` and
/// `event_from_fail` methods will assemble an entire event with all the
/// causes of a failure, however for certain more complex situations where
/// fails are contained within a non fail error type that might also carry
/// useful information it can be useful to call this method instead.
pub fn exception_from_single_fail<F: Fail + ?Sized>(
    f: &F,
    bt: Option<&failure::Backtrace>,
) -> Exception {
    let dbg = format!("{:?}", f);
    Exception {
        ty: parse_type_from_debug(&dbg).to_owned(),
        value: Some(f.to_string()),
        stacktrace: bt
            // format the stack trace with alternate debug to get addresses
            .map(|bt| format!("{:#?}", bt))
            .and_then(|x| parse_stacktrace(&x)),
        ..Default::default()
    }
}

/// Helper function to create an event from a `failure::Error`.
pub fn event_from_error(err: &failure::Error) -> Event<'static> {
    let mut exceptions: Vec<_> = err
        .iter_chain()
        .enumerate()
        .map(|(idx, cause)| {
            let bt = match cause.backtrace() {
                Some(bt) => Some(bt),
                None if idx == 0 => Some(err.backtrace()),
                None => None,
            };
            exception_from_single_fail(cause, bt)
        })
        .collect();
    exceptions.reverse();

    Event {
        exception: exceptions.into(),
        level: Level::Error,
        ..Default::default()
    }
}

/// Helper function to create an event from a `failure::Fail`.
pub fn event_from_fail<F: Fail + ?Sized>(fail: &F) -> Event<'static> {
    let mut exceptions = vec![exception_from_single_fail(fail, fail.backtrace())];

    let mut ptr: Option<&dyn Fail> = None;
    while let Some(cause) = ptr.map(Fail::cause).unwrap_or_else(|| fail.cause()) {
        exceptions.push(exception_from_single_fail(cause, cause.backtrace()));
        ptr = Some(cause);
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: Level::Error,
        ..Default::default()
    }
}

/// Captures a boxed failure (`failure::Error`).
///
/// This dispatches to the current hub.
pub fn capture_error(err: &Error) -> Uuid {
    Hub::with_active(|hub| FailureHubExt::capture_error(hub.as_ref(), err))
}

/// Captures a `failure::Fail`.
///
/// This dispatches to the current hub.
pub fn capture_fail<F: Fail + ?Sized>(fail: &F) -> Uuid {
    Hub::with_active(|hub| hub.capture_fail(fail))
}

/// Hub extension methods for working with failure.
pub trait FailureHubExt {
    /// Captures a boxed failure (`failure::Error`).
    fn capture_error(&self, err: &Error) -> Uuid;
    /// Captures a `failure::Fail`.
    fn capture_fail<F: Fail + ?Sized>(&self, fail: &F) -> Uuid;
}

impl FailureHubExt for Hub {
    fn capture_error(&self, err: &Error) -> Uuid {
        self.capture_event(event_from_error(err))
    }

    fn capture_fail<F: Fail + ?Sized>(&self, fail: &F) -> Uuid {
        self.capture_event(event_from_fail(fail))
    }
}

/// Extension trait providing methods to unwrap a result, preserving backtraces from the
/// underlying error in the event of a panic.
pub trait FailureResultExt {
    /// Type of the success case
    type Value;
    /// Unwraps the result, panicking if it contains an error. Any backtrace attached to the
    /// error will be preserved with the panic.
    fn fallible_unwrap(self) -> Self::Value;
}

impl<T, E> FailureResultExt for Result<T, E>
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
