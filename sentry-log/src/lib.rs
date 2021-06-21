//! Adds support for automatic Breadcrumb and Event capturing from logs.
//!
//! The `log` crate is supported in two ways. First, logs can be captured as
//! breadcrumbs for later. Secondly, error logs can be captured as events to
//! Sentry. By default anything above `Info` is recorded as a breadcrumb and
//! anything above `Error` is captured as error event.
//!
//! # Examples
//!
//! ```
//! let mut log_builder = pretty_env_logger::formatted_builder();
//! log_builder.parse_filters("info");
//! let logger = sentry_log::SentryLogger::with_dest(log_builder.build());
//!
//! log::set_boxed_logger(Box::new(logger)).unwrap();
//! log::set_max_level(log::LevelFilter::Info);
//!
//! let _sentry = sentry::init(());
//!
//! log::info!("Generates a breadcrumb");
//! log::error!("Generates an event");
//! ```
//!
//! Or one might also set an explicit filter, to customize how to treat log
//! records:
//!
//! ```
//! use sentry_log::LogFilter;
//!
//! let logger = sentry_log::SentryLogger::new().filter(|md| match md.level() {
//!     log::Level::Error => LogFilter::Event,
//!     _ => LogFilter::Ignore,
//! });
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

mod converters;
mod logger;

pub use converters::*;
pub use logger::*;
