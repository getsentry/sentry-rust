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
    "sentry_backtrace::",
    // these are not modules but things like __rust_maybe_catch_panic
    "__rust_",
    "___rust_",
    "rust_begin_unwind",
    "_start",
    // these are well-known library frames
    "anyhow::",
    "log::",
    "tokio::",
    "tracing_core::",
    "futures_core::",
    "futures_util::",
];

/// Checks if a function is from a module that shall be considered not in-app by default
pub fn is_well_known_not_in_app(func: &str) -> bool {
    WELL_KNOWN_NOT_IN_APP
        .iter()
        .any(|m| function_starts_with(func, m))
}
