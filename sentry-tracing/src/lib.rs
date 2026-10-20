//! Capture `tracing` spans and events as Sentry traces and structured logs.
//!
//! For most applications, we recommend installing only these two layers:
//!
//! - [`SpanLayer`] captures `tracing` spans as Sentry transactions and spans.
//! - [`LogLayer`] captures `tracing` events as Sentry structured logs. Requires the `logs` feature.
//!
//! We also provide the following layers, but recommend the span and log layers above for most
//! applications:
//!
//! - [`ErrorLayer`] captures `tracing` events as Sentry error events, with support for tags and
//!   exception information.
//! - [`BreadcrumbLayer`] adds `tracing` events as breadcrumbs on subsequent Sentry error events.
//!
//! For migration from the legacy combined layer, see [`SentryLayer`].
//!
//! # Getting started
//!
//! Enable the `tracing` and `logs` features on `sentry` to use its tracing integration re-export.
//! Set a traces sample rate to capture transactions, then install the span and log layers with
//! explicit level filters:
//!
//! ```
//! # #[cfg(feature = "logs")]
//! # {
//! use sentry::integrations::tracing::{LogLayer, SpanLayer};
//! use tracing_subscriber::filter::LevelFilter;
//! use tracing_subscriber::prelude::*;
//!
//! let _guard = sentry::init(
//!     sentry::ClientOptions::new()
//!         // Use a lower sample rate in production.
//!         .traces_sample_rate(1.0),
//! );
//!
//! tracing_subscriber::registry()
//!     .with(SpanLayer::new().with_filter(LevelFilter::INFO))
//!     .with(LogLayer::new().with_filter(LevelFilter::INFO))
//!     .init();
//! # }
//! ```
//!
//! The layers do not filter by level internally. The filters above capture `INFO`, `WARN`, and
//! `ERROR` telemetry, excluding `DEBUG` and `TRACE`. Adjust each layer's filter independently to
//! control volume. Global subscriber filters also apply and can suppress telemetry before the
//! Sentry layers receive it.
//!
//! # Spans
//!
//! `SpanLayer` creates a Sentry transaction when no Sentry span is active, or a child span
//! otherwise. For example,
//! [`tracing::instrument`](https://docs.rs/tracing/latest/tracing/attr.instrument.html) creates
//! spans for instrumented functions; arguments become span data unless skipped.
//!
//! With the log layer installed, the `info!` event below produces a log associated with the same
//! trace as the instrumented functions:
//!
//! ```
//! #[tracing::instrument]
//! async fn outer() {
//!     inner(42).await;
//! }
//!
//! #[tracing::instrument]
//! async fn inner(value: i32) {
//!     tracing::info!(value, "completed step");
//! }
//! ```
//!
//! By default, the Sentry span name matches the `tracing` span name, and its operation is
//! `<target>::<span name>`.
//!
//! ## Special span fields
//!
//! - `sentry.name` overrides the Sentry span name, including when recorded after creation.
//! - `sentry.op` overrides the Sentry span operation, including when recorded after creation.
//! - `sentry.trace` continues the trace from a `sentry-trace` header. It must be set when a
//!   **root Sentry span** is created, meaning no Sentry span is active in the current scope.
//!   Applying it to a child Sentry span or recording it after creation has no effect.
//!
//! ```
//! #[tracing::instrument(skip_all, fields(
//!     sentry.name = "GET /payments",
//!     sentry.op = "http.server",
//!     sentry.trace = headers.get("sentry-trace").unwrap_or(&"".to_owned()),
//! ))]
//! async fn handle_request(headers: std::collections::HashMap<String, String>) {
//!     // ...
//! }
//! ```
//!
//! ## Spans Finishing After Their Transaction
//!
//! The integration finishes a Sentry span when its `tracing` span closes. A `tracing` span only
//! closes once all handles to it have been dropped; exiting the span, or returning from an
//! `#[instrument]`ed function, is not enough. Some libraries keep a clone of the current span for
//! a while, for example by passing [`Span::current()`] to a worker thread, as `sqlx`'s SQLite
//! driver does.
//!
//! If such a clone is dropped after the span's transaction was already sent, for example because
//! the transaction was finished manually or by an integration such as `sentry-tower`, the span is
//! not sent to Sentry. With the [`debug`] client option enabled, the SDK logs a message whenever it
//! drops a span for this reason.
//!
//! To make sure such a span is sent, you can create it with the Sentry API instead and finish it
//! before the transaction finishes:
//!
//! ```
//! # async fn run_query() {}
//! async fn query() {
//!     let span = sentry::configure_scope(|scope| scope.get_span())
//!         .map(|parent| parent.start_child("db.sql.query", "SELECT 1"));
//!
//!     run_query().await;
//!
//!     if let Some(span) = span {
//!         span.finish();
//!     }
//! }
//! ```
//!
//! [`Span::current()`]: https://docs.rs/tracing/latest/tracing/struct.Span.html#method.current
//! [`debug`]: sentry_core::ClientOptions::debug
//!
//! # Logs
//!
//! `LogLayer` sends events accepted by its filter as structured logs. Event fields become log
//! attributes.
//!
//! ```
//! tracing::info!(number = 42, my.key = "value", "Processed request");
//! ```

#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]

mod converters;
mod layer;

#[cfg(feature = "logs")]
#[expect(deprecated, reason = "re-exporting deprecated legacy mapper helpers")]
pub use converters::log_from_event;
#[expect(deprecated, reason = "re-exporting deprecated legacy mapper helpers")]
pub use converters::{breadcrumb_from_event, event_from_event};
pub use layer::*;

const TAGS_PREFIX: &str = "tags.";
const SENTRY_OP_FIELD: &str = "sentry.op";
const SENTRY_NAME_FIELD: &str = "sentry.name";
const SENTRY_TRACE_FIELD: &str = "sentry.trace";
