//! Adds support for automatic Breadcrumb, Event, and Log capturing from `log` records.
//!
//! The `log` crate is supported in three ways:
//! - Records can be captured as Sentry events. These are grouped and show up in the Sentry
//!   [issues](https://docs.sentry.io/product/issues/) page, representing high severity issues to be
//!   acted upon.
//! - Records can be captured as [breadcrumbs](https://docs.sentry.io/product/issues/issue-details/breadcrumbs/).
//!   Breadcrumbs create a trail of what happened prior to an event, and are therefore sent only when
//!   an event is captured, either manually through e.g. `sentry::capture_message` or through integrations
//!   (e.g. the panic integration is enabled (default) and a panic happens).
//! - Records can be captured as traditional [logs](https://docs.sentry.io/product/explore/logs/)
//!   Logs can be viewed and queried in the Logs explorer.
//!
//! By default anything above `Info` is recorded as a breadcrumb and
//! anything above `Error` is captured as error event.
//!
//! To capture records as Sentry logs:
//! 1. Enable the `logs` feature of the `sentry` crate.
//! 2. Initialize the SDK with `enable_logs: true` in your client options.
//! 3. Set up a custom filter (see below) to map records to logs (`LogFilter::Log`) based on criteria such as severity.
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
//!
//! # Sending multiple items to Sentry
//!
//! To map a log record to multiple items in Sentry, you can combine multiple log filters
//! using the bitwise or operator:
//!
//! ```
//! use sentry_log::LogFilter;
//!
//! let logger = sentry_log::SentryLogger::new().filter(|md| match md.level() {
//!     log::Level::Error => LogFilter::Event | LogFilter::Log,
//!     log::Level::Warn => LogFilter::Breadcrumb | LogFilter::Log,
//!     _ => LogFilter::Ignore,
//! });
//! ```
//!
//! If you're using a custom record mapper instead of a filter, use `RecordMapping::Combined`.

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

mod converters;
mod logger;

pub use converters::*;
pub use logger::*;
