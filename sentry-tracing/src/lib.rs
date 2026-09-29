//! Capture `tracing` telemetry with four independently selectable tracing layers:
//!
//! - [`span_layer`](https://docs.rs/sentry-tracing/latest/sentry_tracing/fn.span_layer.html) captures `ERROR`, `WARN`, and `INFO` spans as Sentry transactions and spans.
//! - [`log_layer`](https://docs.rs/sentry-tracing/latest/sentry_tracing/fn.log_layer.html) captures `ERROR`, `WARN`, and `INFO` events as Sentry structured logs.
//! - [`error_layer`](https://docs.rs/sentry-tracing/latest/sentry_tracing/fn.error_layer.html) captures `ERROR` events as Sentry error events (issues).
//! - [`breadcrumb_layer`](https://docs.rs/sentry-tracing/latest/sentry_tracing/fn.breadcrumb_layer.html) adds `WARN` and `INFO` events as breadcrumbs to the current scope.
//!
//! For most applications, we recommend using the span and log layers.
//!
//! # Getting started
//!
//! Set a traces sample rate to capture transactions, then install the layers you would like to
//! use.
//!
//! ```
//! # #[cfg(feature = "logs")]
//! # {
//! use tracing_subscriber::prelude::*;
//!
//! let _guard = sentry::init(
//!     sentry::ClientOptions::new()
//!         // Use a lower sample rate in production.
//!         .traces_sample_rate(1.0),
//! );
//!
//! tracing_subscriber::registry()
//!     .with(sentry::integrations::tracing::span_layer())
//!     .with(sentry::integrations::tracing::log_layer())
//!     .init();
//! # }
//! ```
//!
//! When using the error and breadcrumb layers, we recommend installing the breadcrumb layer before
//! the error layer, so any breadcrumbs added by the breadcrumb layer appear on the correct error.
//!
//! # Spans
//!
//! `span_layer()` creates a Sentry transaction for a root `tracing` span and Sentry spans for
//! captured children. For example, [`tracing::instrument`](https://docs.rs/tracing/latest/tracing/attr.instrument.html)
//! creates spans for instrumented functions; arguments become span data unless skipped.
//!
//! ```
//! #[tracing::instrument]
//! async fn outer() {
//!     inner(42).await;
//! }
//! ```
//!
//! If you also have the recommended `log_layer` installed, then the following will create an
//! info-level log associated with the same trace as the span for the function:
//!
//! ```
//! #[tracing::instrument]
//! async fn inner(value: i32) {
//!     tracing::info!(value, "completed step");
//! }
//! ```
//!
//! By default, the Sentry span name matches the `tracing` span name, and its operation is
//! `<target>::<span name>`. Spans at `DEBUG` and `TRACE` are not captured by default.
//!
//! ## Special span fields
//!
//! - `sentry.name` overrides the Sentry span name, including when recorded after creation.
//! - `sentry.op` overrides the Sentry span operation, including when recorded after creation.
//! - `sentry.trace` continues the trace from a `sentry-trace` header. It must be set when a
//!   **root** span is created; recording it later has no effect.
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
//! # Logs
//!
//! `log_layer()` sends `ERROR`, `WARN`, and `INFO` events as structured logs. Event fields
//! become searchable log attributes; fields with dots appear nested under their common
//! prefix in the Sentry Logs explorer.
//!
//! ```
//! tracing::info!(number = 42, my.key = "value", "Processed request");
//! ```
//!
//! # Errors
//!
//! `error_layer()` captures an `ERROR` event as a Sentry error event. Fields become context;
//! fields prefixed with `tags.` become Sentry tags.
//!
//! ```
//! tracing::error!(field = "value", tags.custom = "value", "Request failed");
//! ```
//!
//! To capture an error chain as Sentry exceptions, record an error trait object. A message
//! can also be provided for grouping and issue context:
//!
//! ```
//! use std::error::Error;
//!
//! let error = std::io::Error::other("connection refused");
//! tracing::error!(error = &error as &dyn Error, "Request failed");
//! ```
//!
//! # Breadcrumbs
//!
//! `breadcrumb_layer()` adds `WARN` and `INFO` events to the current Sentry scope. Breadcrumbs
//! are sent with subsequent captured errors; they are not sent independently. Event fields
//! become breadcrumb data.
//!
//! ```
//! tracing::info!(request_id = 42, "Starting request");
//! tracing::warn!("Retrying request");
//! ```

#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]

mod converters;
mod layer;

pub use converters::*;
pub use layer::*;

const TAGS_PREFIX: &str = "tags.";
const SENTRY_OP_FIELD: &str = "sentry.op";
const SENTRY_NAME_FIELD: &str = "sentry.name";
const SENTRY_TRACE_FIELD: &str = "sentry.trace";
