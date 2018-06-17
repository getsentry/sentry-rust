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
//! the ecosystem a feature flag needs to be enabled.  For the available
//! integrations and how to use them see [integrations](integrations/index.html).
//!
//! # Scope Management
//!
//! Data is typically bound to a scope.  A new scope can be introduced by pushing it
//! with the [`push_scope`](fn.push_scope.html) function.  That scope can then be
//! configured with [`configure_scope`](fn.configure_scope.html) which lets you
//! attach data to it that will be sent along with errors.
//!
//! If a new scope is pushed the data and currently bound client are inherited.  To
//! propagate that scope to a completely different thread a
//! [`scope_handle`](fn.scope_handle.html) can be acquired and passed to a thread
//! where it can be bound.
//!
//! # Minimal API
//!
//! This crate can also be used in "minimal" mode.  This is enabled by disabling all
//! default features of the crate.  In that mode a minimal API set is retained that
//! can be used to instrument code for Sentry without actually using Sentry.  The
//! minimal API is a small set of APIs that dispatch to the underlying implementations on
//! the configured Sentry client.  If the client is not there the minimal API will blackhole
//! a lot of operations.
//!
//! Only if a user then also uses and configures Sentry this code becomes used.
//!
//! In minimal mode some types are restricted in functionality.  For instance the
//! `Client` is not available and the `Hub` does not retain all API functionality.
//! To see what the APIs in mnimal mode look like you can build the docs for this
//! crate without any features enabled.
//!
//! # Features
//!
//! Functionality of the crate can be turned on and off by feature flags.  This is the
//! current list of feature flags:
//!
//! default flags:
//!
//! * `with_client_implementation`: turns on the real client implementation.
//! * `with_backtrace`: enables backtrace support (automatically turned on in a few cases)
//! * `with_panic`: enables the panic integration
//! * `with_failure`: enables the `failure` integration
//! * `with_log`: enables the `log` integration
//! * `with_env_logger`: enables the `env_logger` integration
//! * `with_device_info`: enables the device info context
//! * `with_rust_info`: enables the rust compiler info context
//! * `with_debug_meta`: enables debug meta support (permits server side symbolication)
//!
//! additional features:
//!
//! * `with_error_chain`: enables the error-chain integration
//! * `with_test_support`: enables the test support module
//!
//! # Threading
//!
//! The thread that calls `sentry::init` initializes the first hub which then automatically
//! becomes the base of new hubs (You can get that hub by calling `Hub::main()`).  If a
//! new thread is spawned it gets a new hub based on that one (the thread calls
//! `Hub::new_from_top(Hub::main())`).  The current thread's hub is returned from
//! `Hub::current()`.  Any hub that is wrapped in an `Arc` can be temporarily bound to a
//! thread with `Hub::run_bound`.  For more information see [`Hub`](struct.Hub.html).
#![warn(missing_docs)]

#[cfg(feature = "with_backtrace")]
extern crate backtrace;
#[cfg(feature = "with_client_implementation")]
extern crate im;
#[cfg(
    any(
        feature = "with_backtrace",
        feature = "with_client_implementation",
        feature = "with_failure",
        feature = "with_device_info"
    )
)]
#[macro_use]
extern crate lazy_static;
#[cfg(feature = "with_client_implementation")]
extern crate reqwest;
extern crate sentry_types;
extern crate serde;
extern crate serde_json;
#[cfg(feature = "with_client_implementation")]
extern crate url;
extern crate uuid;

#[cfg(feature = "with_device_info")]
extern crate libc;

#[cfg(feature = "with_device_info")]
extern crate hostname;

#[cfg(all(feature = "with_device_info", not(windows)))]
extern crate uname;

#[cfg(any(feature = "with_backtrace", feature = "with_device_info"))]
extern crate regex;

#[cfg(feature = "with_failure")]
extern crate failure;

#[cfg(feature = "with_error_chain")]
extern crate error_chain;

#[cfg(feature = "with_log")]
extern crate log;

#[cfg(feature = "with_env_logger")]
extern crate env_logger;

#[cfg(feature = "with_debug_meta")]
extern crate findshlibs;

#[macro_use]
mod macros;

mod api;
#[cfg(feature = "with_client_implementation")]
mod client;
mod hub;
mod scope;

#[cfg(feature = "with_backtrace")]
mod backtrace_support;
#[cfg(feature = "with_client_implementation")]
mod constants;
pub mod integrations;
#[cfg(feature = "with_client_implementation")]
mod transport;
#[cfg(feature = "with_client_implementation")]
pub mod utils;
#[cfg(any(test, feature = "with_test_support"))]
pub mod test;

pub use api::*;
