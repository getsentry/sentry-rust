//! Adds support for the error-chain crate.
//!
//! Errors created by the `error-chain` crate can be logged with the
//! `error_chain` integration.
//!
//! # Example
//!
//! ```no_run
//! # #[macro_use] extern crate error_chain;
//! # error_chain! {}
//! use sentry_error_chain::{capture_error_chain, ErrorChainIntegration};
//! # fn function_that_might_fail() -> Result<()> { Ok(()) }
//! # fn test() -> Result<()> {
//! let _sentry = sentry::init(sentry::ClientOptions::default()
//!     .add_integration(ErrorChainIntegration));
//! let result = match function_that_might_fail() {
//!     Ok(result) => result,
//!     Err(err) => {
//!         capture_error_chain(&err);
//!         return Err(err);
//!     }
//! };
//! # Ok(()) }
//! ```

use std::fmt::{Debug, Display};

use error_chain::ChainedError;

use sentry_backtrace::{backtrace_to_stacktrace, error_typename};
use sentry_core::internals::Uuid;
use sentry_core::protocol::{Event, Exception, Level};
use sentry_core::utils::parse_type_name;
use sentry_core::{ClientOptions, Hub, Integration};

fn exceptions_from_error_chain<T>(error: &T) -> Vec<Exception>
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    let mut rv = vec![];

    let (module, ty) = parse_type_name(&error_typename(error.kind()));
    rv.push(Exception {
        ty,
        module,
        value: Some(error.kind().to_string()),
        stacktrace: error_chain::ChainedError::backtrace(error).and_then(backtrace_to_stacktrace),
        ..Default::default()
    });

    for error in error.iter().skip(1) {
        let (module, ty) = parse_type_name(&error_typename(error));
        rv.push(Exception {
            ty,
            module,
            value: Some(error.to_string()),
            ..Default::default()
        })
    }

    rv
}

/// Creates an event from an error chain.
pub fn event_from_error_chain<T>(e: &T) -> Event<'static>
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    Event {
        exception: exceptions_from_error_chain(e).into(),
        level: Level::Error,
        ..Default::default()
    }
}

/// Captures an error chain.
pub fn capture_error_chain<T>(e: &T) -> Uuid
where
    T: ChainedError,
    T::ErrorKind: Debug + Display,
{
    Hub::with_active(|hub| hub.capture_error_chain(e))
}

/// Hub extension methods for working with error chain
pub trait ErrorChainHubExt {
    /// Captures an error chain on a specific hub.
    fn capture_error_chain<T>(&self, e: &T) -> Uuid
    where
        T: ChainedError,
        T::ErrorKind: Debug + Display;
}

impl ErrorChainHubExt for Hub {
    fn capture_error_chain<T>(&self, e: &T) -> Uuid
    where
        T: ChainedError,
        T::ErrorKind: Debug + Display,
    {
        self.capture_event(event_from_error_chain(e))
    }
}

pub struct ErrorChainIntegration;

impl Integration for ErrorChainIntegration {
    fn name(&self) -> &'static str {
        "error-chain"
    }

    fn setup(&self, cfg: &mut ClientOptions) {
        cfg.in_app_exclude.push("error_chain::");
        cfg.extra_border_frames.push("error_chain::make_backtrace");
    }
}
