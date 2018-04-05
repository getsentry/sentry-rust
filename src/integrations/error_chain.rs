//! Adds support for the error-chain crate.
//!
//! This module is available if the crate is compiled with the `with_error_chain` feature.  It's
//! not enabled by default as error-chain is being replaced by `failure`.
use std::fmt::{Debug, Display};
use error_chain::ChainedError;

use api::protocol::{Event, Level, Exception, Frame, InstructionInfo, FileLocation};
use backtrace_support::{error_typename, filename, sanitize_symbol};

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
        stacktrace: error.backtrace().and_then(|backtrace| {
            let frames = backtrace
                .frames()
                .iter()
                .flat_map(|frame| {
                    frame.symbols().iter().map(move |sym| {
                        let abs_path = sym.filename().map(|m| m.to_string_lossy().to_string());
                        let filename = abs_path.as_ref().map(|p| filename(p));
                        let symbol = sym.name().map_or("<unknown>".into(), |n| n.to_string());
                        let function = sanitize_symbol(&symbol).to_string();
                        Frame {
                            symbol: if symbol != function {
                                Some(symbol)
                            } else {
                                None
                            },
                            function: Some(function),
                            instruction_info: InstructionInfo {
                                instruction_addr: Some(frame.ip().into()),
                                ..Default::default()
                            },
                            location: FileLocation {
                                abs_path: abs_path,
                                filename: filename,
                                line: sym.lineno().map(|l| l as u64),
                                column: None,
                            },
                            ..Default::default()
                        }
                    })
                })
                .collect();
            Stacktrace::from_frames_reversed(frames)
        }),
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
