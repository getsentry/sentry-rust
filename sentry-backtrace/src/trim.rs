use sentry_core::protocol::{Frame, Stacktrace};

use crate::utils::function_starts_with;

const WELL_KNOWN_NOT_IN_APP: &[&str] = &[
    // standard library and sentry crates
    "std::",
    "core::",
    "alloc::",
    "backtrace::",
    "sentry::",
    "sentry_core::",
    "sentry_types::",
    // these are not modules but things like __rust_maybe_catch_panic
    "__rust_",
    "___rust_",
    "_rust_begin_unwind",
    // these are well-known library frames
    "anyhow::",
    "log::",
    "tokio::",
    "tracing_core::",
    "futures_core::",
    "futures_util::",
];

const WELL_KNOWN_BORDER_FRAMES: &[&str] = &[
    "std::panicking::begin_panic",
    "core::panicking::panic",
    // well-known library frames
    "anyhow::",
    "<sentry_log::Logger as log::Log>::log",
    "tracing_core::",
];

/// A helper function to trim a stacktrace.
pub fn trim_stacktrace<F>(stacktrace: &mut Stacktrace, f: F)
where
    F: Fn(&Frame, &Stacktrace) -> bool,
{
    let known_cutoff = stacktrace
        .frames
        .iter()
        .rev()
        .position(|frame| match frame.function {
            Some(ref func) => is_well_known_border_frame(func) || f(frame, stacktrace),
            None => false,
        });

    if let Some(cutoff) = known_cutoff {
        let trunc = stacktrace.frames.len() - cutoff - 1;
        stacktrace.frames.truncate(trunc);
    }
}

/// Checks if a function is from a module that shall be considered not in-app by default
pub fn is_well_known_not_in_app(func: &str) -> bool {
    WELL_KNOWN_NOT_IN_APP
        .iter()
        .any(|m| function_starts_with(func, m))
}

/// Checks if a function is a well-known border frame
fn is_well_known_border_frame(func: &str) -> bool {
    WELL_KNOWN_BORDER_FRAMES
        .iter()
        .any(|m| function_starts_with(func, m))
}
