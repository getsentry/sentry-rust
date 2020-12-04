//! Sentry `slog` Integration.
//!
//! This mainly provides the [`SentryDrain`], which wraps another [`slog::Drain`]
//! and can be configured to forward [`slog::Record`]s to Sentry.
//! The [`SentryDrain`] can be used to create a `slog::Logger`.
//!
//! The integration also supports [`slog::KV`] pairs. They will be added to the
//! breadcrumb `data` or the event `extra` properties respectively.
//!
//! # Examples
//!
//! ```
//! use sentry_slog::SentryDrain;
//!
//! let _sentry = sentry::init(());
//!
//! let drain = SentryDrain::new(slog::Discard);
//! let root = slog::Logger::root(drain, slog::o!("global_kv" => 1234));
//!
//! # let events = sentry::test::with_captured_events(|| {
//! slog::info!(root, "recorded as breadcrumb"; "breadcrumb_kv" => Some("breadcrumb"));
//! slog::warn!(root, "recorded as regular event"; "event_kv" => "event");
//! # });
//! # let captured_event = events.into_iter().next().unwrap();
//!
//! let breadcrumb = &captured_event.breadcrumbs.as_ref()[0];
//! assert_eq!(
//!     breadcrumb.message.as_deref(),
//!     Some("recorded as breadcrumb")
//! );
//! assert_eq!(breadcrumb.data["breadcrumb_kv"], "breadcrumb");
//! assert_eq!(breadcrumb.data["global_kv"], 1234);
//!
//! assert_eq!(
//!     captured_event.message.as_deref(),
//!     Some("recorded as regular event")
//! );
//! assert_eq!(captured_event.extra["event_kv"], "event");
//! assert_eq!(captured_event.extra["global_kv"], 1234);
//!
//! # let events = sentry::test::with_captured_events(|| {
//! slog::crit!(root, "recorded as exception event");
//! # });
//! # let captured_event = events.into_iter().next().unwrap();
//!
//! assert_eq!(
//!     captured_event.message.as_deref(),
//!     Some("recorded as exception event")
//! );
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

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

mod converters;
mod drain;

pub use converters::*;
pub use drain::{default_filter, LevelFilter, RecordMapping, SentryDrain};
