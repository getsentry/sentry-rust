//! <p style="margin: -10px 0 0 15px; padding: 0; float: right;">
//!   <a href="https://sentry.io/"><img
//!     src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png"
//!     style="width: 260px"></a>
//! </p>
//!
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
//! # Integrations
//!
//! What makes this crate useful are the various integrations that exist.  Some
//! of them are enabled by default, some uncommon ones or for deprecated parts of
//! the ecosystem a feature flag needs to be enabled.
//!
//! ## Panic Handler
//!
//! **Feature:** *always available*
//!
//! A panic handler can be installed that will automatically dispatch all errors
//! to Sentry that are caused by a panic:
//!
//! ```
//! use sentry::integrations::panic::register_panic_handler;
//! register_panic_handler();
//! ```
//!
//! ## Failure
//!
//! **Feature:** *enabled by default* (`with_failure`)
//!
//! Failure errors and `Fail` objects can be logged with the failure integration.
//! This works really well if you use the `failure::Error` type or if you have
//! `failure::Fail` objects that use the failure context internally to gain a
//! backtrace.
//!
//! Example usage:
//!
//! ```
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
//!
//! ## Log
//!
//! **Feature:** *enabled by default* (`with_log`)
//!
//! The `log` crate is supported in two ways.  First events can be captured as
//! breadcrumbs for later, secondly error events can be logged as events to
//! Sentry.  By default anything above `Info` is recorded as breadcrumb and
//! anything above `Error` is captured as error event.
//!
//! However due to how log systems in Rust work this currently requires you to
//! slightly change your log setup.  This is an example with the pretty
//! env logger crate:
//!
//! ```
//! # extern crate sentry;
//! # extern crate pretty_env_logger;
//! let mut log_builder = pretty_env_logger::formatted_builder().unwrap();
//! log_builder.parse("info");  // or env::var("RUST_LOG")
//! sentry::integrations::log::init(Some(
//!     Box::new(log_builder.build())), Default::default());
//! ```
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

#[cfg(feature = "with_device_info")]
extern crate libc;

#[cfg(feature = "with_device_info")]
extern crate hostname;

#[cfg(all(feature = "with_device_info", not(windows)))]
extern crate uname;

#[cfg(feature = "with_failure")]
extern crate regex;

#[cfg(feature = "with_failure")]
extern crate failure;

#[cfg(feature = "with_error_chain")]
extern crate error_chain;

#[cfg(feature = "with_log")]
extern crate log;

#[macro_use]
mod macros;

mod client;
mod constants;
mod transport;
mod scope;
mod api;
pub mod utils;
pub mod integrations;
mod backtrace_support;

pub use api::*;
