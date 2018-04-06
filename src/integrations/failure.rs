//! Adds support for the failure crate.
//!
//! This module is available if the crate is compiled with the `with_failure` feature.  It's
//! part of the default features of the crate.
use uuid::Uuid;
use regex::Regex;
use failure;
use failure::{Error, Fail};

use api::protocol::{Event, Exception, FileLocation, Frame, InstructionInfo, Level, Stacktrace};
use backtrace_support::{demangle_symbol, error_typename, filename, strip_symbol};
use scope::with_client_and_scope;

lazy_static! {
    static ref FRAME_RE: Regex = Regex::new(r#"(?xm)
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
    "#).unwrap();
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
                    abs_path: abs_path,
                    filename: filename,
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

fn single_fail_to_exception(f: &Fail, bt: Option<&failure::Backtrace>) -> Exception {
    Exception {
        ty: error_typename(f),
        value: Some(f.to_string()),
        stacktrace: bt.map(|backtrace| backtrace.to_string())
            .and_then(|x| parse_stacktrace(&x)),
        ..Default::default()
    }
}

/// Helper function to create an event from a `failure::Error`.
pub fn event_from_error(err: &failure::Error) -> Event {
    let mut exceptions = vec![];
    for (idx, cause) in err.causes().enumerate() {
        let bt = match cause.backtrace() {
            Some(bt) => Some(bt),
            // TODO: not 0, but effectively -1
            None if idx == 0 => Some(err.backtrace()),
            None => None,
        };
        exceptions.push(single_fail_to_exception(cause, bt));
    }
    Event {
        exceptions,
        level: Level::Error,
        ..Default::default()
    }
}

/// Helper function to create an event from a `failure::Fail`.
pub fn event_from_fail<F>(fail: &F) -> Event
where
    F: Fail + Sized,
{
    Event {
        exceptions: failure::Fail::causes(fail)
            .map(|cause| single_fail_to_exception(cause, cause.backtrace()))
            .collect(),
        level: Level::Error,
        ..Default::default()
    }
}

/// Captures a boxed failure (`failure::Error`).
pub fn capture_fail_error(err: &Error) -> Uuid {
    with_client_and_scope(|client, scope| client.capture_event(event_from_error(err), Some(scope)))
}

/// Captures a `failure::Fail`.
pub fn capture_fail<F>(fail: &F) -> Uuid
where
    F: Fail + Sized,
{
    with_client_and_scope(|client, scope| client.capture_event(event_from_fail(fail), Some(scope)))
}
