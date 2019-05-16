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
use failure::{Error, Fail};
use regex::Regex;

use crate::backtrace_support::{demangle_symbol, error_typename, filename, strip_symbol};
use crate::hub::Hub;
use crate::internals::Uuid;
use crate::protocol::{Event, Exception, Frame, Level, Stacktrace};

lazy_static::lazy_static! {
    static ref MODULE_SPLIT_RE: Regex = Regex::new(r"^(.*)::(.*?)$").unwrap();
    static ref FRAME_RE: Regex = Regex::new(
        r#"(?xm)
        ^
            \s*(?:\d+:)?\s*                      # frame number (missing for inline)

            (?:
                (?P<addr_old>0x[a-f0-9]+)        # old style address prefix
                \s-\s
            )?

            (?P<symbol>[^\r\n\(]+)               # symbol name

            (?:
                \s\((?P<addr_new>0x[a-f0-9]+)\)  # new style address in parens
            )?

            (?:
                \r?\n
                \s+at\s                          # padded "at" in new line
                (?P<path>[^\r\n]+?)              # path to source file
                (?::(?P<lineno>\d+))?            # optional source line
            )?
        $
    "#
    )
    .unwrap();
}

fn parse_stacktrace(bt: &str) -> Option<Stacktrace> {
    let mut last_address = None;

    let frames = FRAME_RE
        .captures_iter(&bt)
        .map(|captures| {
            let abs_path = captures.name("path").map(|m| m.as_str().to_string());
            let filename = abs_path.as_ref().map(|p| filename(p));
            let real_symbol = captures["symbol"].to_string();
            let symbol = strip_symbol(&real_symbol);
            let function = demangle_symbol(symbol);

            // Obtain the instruction address. A missing address usually indicates an inlined stack
            // frame, in which case the previous address needs to be used.
            last_address = captures
                .name("addr_new")
                .or_else(|| captures.name("addr_old"))
                .and_then(|m| m.as_str().parse().ok())
                .or(last_address);

            Frame {
                symbol: if symbol != function {
                    Some(symbol.into())
                } else {
                    None
                },
                function: Some(function),
                instruction_addr: last_address,
                abs_path,
                filename,
                lineno: captures
                    .name("lineno")
                    .map(|x| x.as_str().parse::<u64>().unwrap()),
                ..Default::default()
            }
        })
        .collect();

    Stacktrace::from_frames_reversed(frames)
}

fn fail_typename<F: Fail + ?Sized>(f: &F) -> (Option<String>, String) {
    if let Some(name) = f.name() {
        if let Some(caps) = MODULE_SPLIT_RE.captures(name) {
            (Some(caps[1].to_string()), caps[2].to_string())
        } else {
            (None, name.to_string())
        }
    } else {
        (None, error_typename(f))
    }
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
    let (module, ty) = fail_typename(f);
    Exception {
        ty,
        module,
        value: Some(f.to_string()),
        stacktrace: bt
            .map(failure::Backtrace::to_string)
            .and_then(|x| parse_stacktrace(&x)),
        ..Default::default()
    }
}

/// Helper function to create an event from a `failure::Error`.
pub fn event_from_error(err: &failure::Error) -> Event<'static> {
    let mut exceptions = vec![];

    for (idx, cause) in err.iter_chain().enumerate() {
        let bt = match cause.backtrace() {
            Some(bt) => Some(bt),
            None if idx == 0 => Some(err.backtrace()),
            None => None,
        };
        exceptions.push(exception_from_single_fail(cause, bt));
    }

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
    Hub::with_active(|hub| hub.capture_error(err))
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

#[test]
fn test_parse_stacktrace() {
    use crate::protocol::Addr;

    let backtrace = r#"
   2: <failure::error::error_impl::ErrorImpl as core::convert::From<F>>::from::h3bae66c036570137 (0x55a12174de62)
             at /root/.cargo/registry/src/github.com-1ecc6299db9ec823/failure-0.1.5/src/error/error_impl.rs:19
      <failure::error::Error as core::convert::From<F>>::from::hc7d0d62dae166cea
             at /root/.cargo/registry/src/github.com-1ecc6299db9ec823/failure-0.1.5/src/error/mod.rs:36
      failure::error_message::err_msg::he322d3ed9409189a
             at /root/.cargo/registry/src/github.com-1ecc6299db9ec823/failure-0.1.5/src/error_message.rs:12
      rust::inline2::h562e5687710b6a71
             at src/main.rs:5
      rust::not_inline::h16f5b6019e5f0815
             at src/main.rs:10
   7: main (0x55e3895a4dc7)
"#;

    let stacktrace = parse_stacktrace(backtrace).expect("stacktrace");
    assert_eq!(stacktrace.frames.len(), 6);

    assert_eq!(stacktrace.frames[0].function, Some("main".into()));
    assert_eq!(
        stacktrace.frames[0].instruction_addr,
        Some(Addr(0x55e3_895a_4dc7))
    );

    // Inlined frame, inherits address from parent
    assert_eq!(
        stacktrace.frames[1].function,
        Some("rust::not_inline".into())
    );
    assert_eq!(
        stacktrace.frames[1].instruction_addr,
        Some(Addr(0x55a1_2174_de62))
    );
}
