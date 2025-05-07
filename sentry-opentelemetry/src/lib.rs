//! OpenTelemetry support for Sentry.
//!
//! This integration allows you to capture spans from your existing OpenTelemetry setup and send
//! them to Sentry, with support for distributed tracing.
//! It's assumed that only the [OpenTelemetry tracing
//! API](https://opentelemetry.io/docs/specs/otel/trace/api/) is used to start/end/modify Spans.
//! Mixing it with the Sentry tracing API (e.g. `sentry_core::start_transaction(ctx)`) will not
//! work, as the spans created with the two methods will not be nested properly.
//! Capturing events (either manually with e.g. `sentry::capture_event`, or automatically with e.g. the
//! `sentry-panic` integration) will send them to Sentry with the correct trace and span
//! association.
//!
//! # Configuration
//!
//! Initialize Sentry, then register the [`SentryPropagator`] and the [`SentrySpanProcessor`]:
//!
//! ```
//! use opentelemetry::{global};
//! use opentelemetry_sdk::{
//!     propagation::TraceContextPropagator, trace::SdkTracerProvider,
//! };
//!
//! // Initialize the Sentry SDK
//! let _guard = sentry::init(sentry::ClientOptions {
//!     // Enable capturing of traces; set this a to lower value in production.
//!     // For more sophisticated behavior use a custom
//!     // [`sentry::ClientOptions::traces_sampler`] instead.
//!     // That's the equivalent of a tail sampling processor in OpenTelemetry.
//!     // These options will only affect sampling of the spans that are sent to Sentry,
//!     // not of the underlying OpenTelemetry spans.
//!     traces_sample_rate: 1.0,
//!     ..sentry::ClientOptions::default()
//! });
//!
//! // Register the Sentry propagator to enable distributed tracing
//! global::set_text_map_propagator(sentry_opentelemetry::SentryPropagator::new());
//!
//! let tracer_provider = SdkTracerProvider::builder()
//!     // Register the Sentry span processor to send OpenTelemetry spans to Sentry
//!     .with_span_processor(sentry_opentelemetry::SentrySpanProcessor::new())
//!     .build();
//!
//! global::set_tracer_provider(tracer_provider);
//! ```

mod converters;
mod processor;
mod propagator;

pub use processor::*;
pub use propagator::*;
