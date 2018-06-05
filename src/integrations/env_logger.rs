//! Adds support for automatic breadcrumb capturing from logs with `env_logger`.
//!
//! **Feature:** `with_env_logger` (*enabled by default*)
//!
//! # Configuration
//!
//! In the most trivial version you call this crate's init function instead of the one
//! from `env_logger` and pass `None` as logger:
//!
//! ```no_run
//! # extern crate sentry;
//! sentry::integrations::env_logger::init(None, Default::default());
//! ```
//!
//! This parses the default `RUST_LOG` environment variable and configures both `env_logger`
//! and this crate appropriately.  If you want to create your own logger you can forward it
//! accordingly:
//!
//! ```no_run
//! # extern crate sentry;
//! # extern crate pretty_env_logger;
//! let mut log_builder = pretty_env_logger::formatted_builder().unwrap();
//! log_builder.parse("info,foo=debug");
//! sentry::integrations::env_logger::init(Some(log_builder.build()), Default::default());
//! ```
use env_logger;

use integrations::log::{self as sentry_log, LoggerOptions};


/// Initializes the environment logger.
///
/// If a logger is given then it is used, otherwise a new logger is created in the same
/// way as `env_logger::init` does normally.  The `global_filter` on the options is set
/// to the filter of the logger.
pub fn init(logger: Option<env_logger::Logger>, mut options: LoggerOptions) {
    let logger = logger.unwrap_or_else(|| {
        env_logger::Builder::from_env(env_logger::Env::default()).build()
    });
    let filter = logger.filter();
    if options.global_filter.is_none() {
        options.global_filter = Some(filter);
    }
    sentry_log::init(Some(Box::new(logger)), options);
}
