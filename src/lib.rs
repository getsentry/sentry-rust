//! This crate provides support for logging events and errors / panics to
//! the [Sentry](https://sentry.io/) error logging service.
//!
//! ## Feature Flags
//!
//! The following feature flags control the behavior of the client:
//!
//! * `with_log` (default): enables support for capturing log messages as breadcrumbs.
//! * `with_failure` (default): enables support for reporting `failure::Fail`
//!   objects as exceptions.
//! * `with_error_chain`: enables logging of error chain errors as exceptions.
#[warn(missing_docs)]
extern crate backtrace;
extern crate futures;
#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate sentry_types;
extern crate serde;
extern crate serde_json;
extern crate url;
extern crate uuid;

#[cfg(feature = "with_failure")]
extern crate regex;

#[cfg(feature = "with_failure")]
extern crate failure;

#[cfg(feature = "with_error_chain")]
extern crate error_chain;

#[cfg(feature = "with_log")]
extern crate log;

mod client;
mod constants;
mod transport;
mod scope;
mod api;
pub mod integrations;
mod backtrace_support;

pub use api::*;
