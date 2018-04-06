//! This crate provides support for logging events and errors / panics to
//! the [Sentry](https://sentry.io/) error logging service.  It integrates with
//! the standard panic system in Rust as well as a few popular error handling
//! setups.
//!
//! # Quickstart
//!
//! To use the crate you need to create a client first.  When a client is created
//! it's typically bound to the current thread by calling `bind_client`.  By default
//! this happens by using the `sentry::init` convenience function.  When the client
//! is bound to the main thread it also becomes the default client for future
//! threads created but it is always possible to override the client for a thread
//! later by explicitly binding it.
//!
//! The `sentry::init` function returns a guard that when dropped will flush
//! Events that were not yet sent to the sentry service.  It has a two second
//! deadline for this so shutdown of applications might slightly delay as a result
//! of this.
//!
//! ```
//! extern crate sentry;
//!
//! fn main() {
//!     let _guard = sentry::init("https://key@sentry.io/42");
//!     sentry::capture_message("Hello World!", sentry::Level::Info);
//!     // when the guard goes out of scope here, the client will wait up to two
//!     // seconds to send remaining events to the service.
//! }
//! ```
//!
//! # Feature Flags
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
extern crate im;
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
pub mod utils;
pub mod integrations;
mod backtrace_support;

pub use api::*;
