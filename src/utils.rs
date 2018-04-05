//! Useful utilities for working with events.
use backtrace;

use api::protocol::Stacktrace;
use backtrace_support::backtrace_to_stacktrace;

/// Returns the current backtrace as sentry stacktrace.
pub fn current_stacktrace() -> Option<Stacktrace> {
    backtrace_to_stacktrace(&backtrace::Backtrace::new())
}
