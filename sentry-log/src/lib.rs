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
//! let log_integration = sentry_log::LogIntegration::default();
//! let _setry = sentry::init(sentry::ClientOptions::default().add_integration(log_integration));
//!
//! log::info!("Generates a breadcrumb");
//! ```
//!
//! Or optionally with env_logger support:
//!
//! ```
//! let mut log_builder = pretty_env_logger::formatted_builder();
//! log_builder.parse_filters("info");
//! let log_integration =
//!     sentry_log::LogIntegration::default().with_env_logger_dest(Some(log_builder.build()));
//! let _setry = sentry::init(sentry::ClientOptions::default().add_integration(log_integration));
//!
//! log::error!("Generates an event");
//! ```

mod converters;
mod integration;
mod logger;

pub use converters::{breadcrumb_from_record, event_from_record};
pub use integration::LogIntegration;
pub use logger::Logger;
