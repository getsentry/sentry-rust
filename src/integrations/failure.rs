//! Adds support for the failure crate.
//!
//! **Feature:** `with_failure` (enabled by default)
//!
//! Failure errors and `Fail` objects can be logged with the failure integration.
//! This works really well if you use the `failure::Error` type or if you have
//! `failure::Fail` objects that use the failure context internally to gain a
//! backtrace.
//!
//! # Example
//!
//! ```no_run
//! # extern crate sentry;
//! # extern crate failure;
//! # fn function_that_might_fail() -> Result<(), failure::Error> { Ok(()) }
//! use sentry::integrations::failure::capture_error;
//! # fn test() -> Result<(), failure::Error> {
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
//! To capture fails and not errors use `capture_fail`.
use failure;
use failure::{Error, Fail};
use regex::Regex;
use uuid::Uuid;

use api::protocol::{Event, Exception, FileLocation, Frame, InstructionInfo, Level, Stacktrace};
use backtrace_support::{demangle_symbol, error_typename, filename, strip_symbol};
use hub::Hub;

lazy_static! {
    static ref FRAME_RE: Regex = Regex::new(
        r#"(?xm)
        ^
            [\ ]*(?:\d+:)[\ ]*                  # leading frame number
            (?P<addr>0x[a-f0-9]+)               # addr
            [\ ]-[\ ]
            (?P<symbol>[^\r\n]+)
            (?:
                \r?\n
                [\ \t]+at[\ ]
                (?P<path>[^\r\n]+?)
                (?::(?P<lineno>\d+))?
            )?
        $
    "#
    ).unwrap();
}

fn parse_stacktrace(bt: &str) -> Option<Stacktrace> {
    let frames = FRAME_RE
        .captures_iter(&bt)
        .map(|captures| {
            let abs_path = captures.name("path").map(|m| m.as_str().to_string());
            let filename = abs_path.as_ref().map(|p| filename(p));
            let real_symbol = captures["symbol"].to_string();
            let symbol = strip_symbol(&real_symbol);
            let function = demangle_symbol(symbol);
            Frame {
                symbol: if symbol != function {
                    Some(symbol.into())
                } else {
                    None
                },
                function: Some(function),
                instruction_info: InstructionInfo {
                    instruction_addr: Some(captures["addr"].parse().unwrap()),
                    ..Default::default()
                },
                location: FileLocation {
                    abs_path,
                    filename,
                    line: captures
                        .name("lineno")
                        .map(|x| x.as_str().parse::<u64>().unwrap()),
                    column: None,
                },
                ..Default::default()
            }
        })
        .collect();

    Stacktrace::from_frames_reversed(frames)
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
    Exception {
        ty: error_typename(f),
        value: Some(f.to_string()),
        stacktrace: bt.map(|backtrace| backtrace.to_string())
            .and_then(|x| parse_stacktrace(&x)),
        ..Default::default()
    }
}

/// Helper function to create an event from a `failure::Error`.
pub fn event_from_error(err: &failure::Error) -> Event<'static> {
    let mut exceptions = vec![];

    for (idx, cause) in err.causes().enumerate() {
        let bt = match cause.backtrace() {
            Some(bt) => Some(bt),
            None if idx == 0 => Some(err.backtrace()),
            None => None,
        };
        exceptions.push(exception_from_single_fail(cause, bt));
    }

    exceptions.reverse();
    Event {
        exceptions,
        level: Level::Error,
        ..Default::default()
    }
}

/// Helper function to create an event from a `failure::Fail`.
pub fn event_from_fail<F: Fail + ?Sized>(fail: &F) -> Event<'static> {
    let mut exceptions = vec![exception_from_single_fail(fail, fail.backtrace())];

    let mut ptr: Option<&Fail> = None;
    while let Some(cause) = ptr.map(Fail::cause).unwrap_or_else(|| fail.cause()) {
        exceptions.push(exception_from_single_fail(cause, cause.backtrace()));
        ptr = Some(cause);
    }

    exceptions.reverse();
    Event {
        exceptions,
        level: Level::Error,
        ..Default::default()
    }
}

/// Captures a boxed failure (`failure::Error`).
///
/// This dispatches to the current hub.
pub fn capture_error(err: &Error) -> Uuid {
    Hub::with_active(|hub| hub.capture_event(event_from_error(err)))
}

/// Captures a `failure::Fail`.
///
/// This dispatches to the current hub.
pub fn capture_fail<F: Fail + ?Sized>(fail: &F) -> Uuid {
    Hub::with_active(|hub| hub.capture_event(event_from_fail(fail)))
}

/// Hub extension methods for working with failure.
pub trait HubExt {
    /// Captures a boxed failure (`failure::Error`).
    fn capture_error(&self, err: &Error) -> Uuid;
    /// Captures a `failure::Fail`.
    fn capture_fail<F: Fail + ?Sized>(&self, fail: &F) -> Uuid;
}

impl HubExt for Hub {
    fn capture_error(&self, err: &Error) -> Uuid {
        self.capture_event(event_from_error(err))
    }

    fn capture_fail<F: Fail + ?Sized>(&self, fail: &F) -> Uuid {
        self.capture_event(event_from_fail(fail))
    }
}
