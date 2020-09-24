//! Adds support for automatic Breadcrumb and Event capturing from logs.
//!
//! The `log` crate is supported in two ways. First, logs can be captured as
//! breadcrumbs for later. Secondly, error logs can be captured as events to
//! Sentry. By default anything above `Info` is recorded as breadcrumb and
//! anything above `Error` is captured as error event.
//!
//! # Examples
//!
//! ```
//! let mut log_builder = pretty_env_logger::formatted_builder();
//! log_builder.parse_filters("info");
//! let logger = sentry_log::SentryLogger::with_dest(log_builder.build());
//!
//! log::set_boxed_logger(Box::new(logger))
//!     .map(|()| log::set_max_level(log::LevelFilter::Info))
//!     .unwrap();
//!
//! let log_integration = sentry_log::LogIntegration::default();
//! let _sentry = sentry::init(sentry::ClientOptions::new()
//!     .add_integration(sentry_log::LogIntegration::new()));
//!
//! log::info!("Generates a breadcrumb");
//! log::error!("Generates an event");
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

mod converters;
mod integration;
mod logger;

pub use converters::*;
pub use integration::LogIntegration;
pub use logger::{NoopLogger, SentryLogger};
