//! Sentry `slog` Integration.
//!
//! This mainly provides the [`SentryDrain`], which wraps another [`slog::Drain`]
//! and can be configured to forward [`slog::Record`]s to Sentry.
//! The [`SentryDrain`] can be used to create a `slog::Logger`.
//!
//! *NOTE*: This integration currently does not process any `slog::KV` pairs,
//! but support for this will be added in the future.
//!
//! # Examples
//!
//! ```
//! use sentry::{init, ClientOptions};
//! use sentry_slog::SentryDrain;
//!
//! let _sentry = sentry::init(());
//!
//! let drain = SentryDrain::new(slog::Discard);
//! let root = slog::Logger::root(drain, slog::o!());
//!
//! # let events = sentry::test::with_captured_events(|| {
//! slog::info!(root, "recorded as breadcrumb");
//! slog::warn!(root, "recorded as regular event");
//! # });
//! # let captured_event = events.into_iter().next().unwrap();
//!
//! assert_eq!(
//!     captured_event.breadcrumbs.as_ref()[0].message.as_deref(),
//!     Some("recorded as breadcrumb")
//! );
//! assert_eq!(
//!     captured_event.message.as_deref(),
//!     Some("recorded as regular event")
//! );
//!
//! # let events = sentry::test::with_captured_events(|| {
//! slog::crit!(root, "recorded as exception event");
//! # });
//! # let captured_event = events.into_iter().next().unwrap();
//!
//! assert_eq!(captured_event.exception.len(), 1);
//! ```
//!
//! The Drain can also be customized with a `filter`, and a `mapper`:
//!
//! ```
//! use sentry_slog::{exception_from_record, LevelFilter, RecordMapping, SentryDrain};
//!
//! let drain = SentryDrain::new(slog::Discard)
//!     .filter(|level| match level {
//!         slog::Level::Critical | slog::Level::Error => LevelFilter::Event,
//!         _ => LevelFilter::Ignore,
//!     })
//!     .mapper(|record, kv| match record.level() {
//!         slog::Level::Critical | slog::Level::Error => {
//!             RecordMapping::Event(exception_from_record(record, kv))
//!         }
//!         _ => RecordMapping::Ignore,
//!     });
//! ```
//!
//! When a `mapper` is specified, a corresponding `filter` should also be
//! provided.
//!
//! [`SentryDrain`]: struct.SentryDrain.html

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

mod converters;
mod drain;

pub use converters::*;
pub use drain::{default_filter, LevelFilter, RecordMapping, SentryDrain};
