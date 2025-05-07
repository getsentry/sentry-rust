<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-opentelemetry

Support for capturing Sentry spans from OpenTelemetry spans.

Sentry spans are automatically captured from OpenTelemetry spans via `SentrySpanProcessor`.
Distributed tracing is supported via `SentryPropagator`.
Note that it's assumed that only the OTEL API is used to create and manage spans.
Mixing the OTEL and Sentry tracing API will not work, and will result in separate traces being captured.
Using the Sentry API for other purposes is supported.
For example, capturing an error inside a span will correctly send it to Sentry with the span association.

If you're using `tracing-opentelemetry`, use `sentry-tracing` instead.

# Configuration

Add the necessary dependencies to your Cargo.toml:

```toml
[dependencies]
opentelemetry = { version = "0.29.1", features = ["trace"] }
opentelemetry_sdk = { version = "0.29.0", features = ["trace"] }
sentry = { version = "0.38.0", features = ["opentelemetry"] }
```

Initialize Sentry with a `traces_sample_rate`, then register the [`SentryPropagator`] and the
[`SentrySpanProcessor`]:

```rust
use opentelemetry::{
    global,
    trace::{TraceContextExt, Tracer},
    KeyValue,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use sentry::integrations::opentelemetry as sentry_opentelemetry;

// Initialize the Sentry SDK
let _guard = sentry::init((
    "https://your-dsn@sentry.io/0",
    sentry::ClientOptions {
        // Enable capturing of traces; set this a to lower value in production.
        // For more sophisticated behavior use a custom
        // [`sentry::ClientOptions::traces_sampler`] instead.
        // That's the equivalent of a tail sampling processor in OpenTelemetry.
        // These options will only affect sampling of the spans that are sent to Sentry,
        // not of the underlying OpenTelemetry spans.
        traces_sample_rate: 1.0,
        debug: true,
        ..sentry::ClientOptions::default()
    },
));

// Register the Sentry propagator to enable distributed tracing
global::set_text_map_propagator(sentry_opentelemetry::SentryPropagator::new());

let tracer_provider = SdkTracerProvider::builder()
    // Register the Sentry span processor to send OpenTelemetry spans to Sentry
    .with_span_processor(sentry_opentelemetry::SentrySpanProcessor::new())
    .build();

global::set_tracer_provider(tracer_provider);
```

# Usage

Use the OpenTelemetry API to create spans. They will be captured by Sentry:

```rust
let tracer = global::tracer("tracer");
// Creates a Sentry span (transaction) with the name set to "example"
tracer.in_span("example", |_| {
    // Creates a Sentry child span with the name set to "child"
    tracer.in_span("child", |cx| {
        // OTEL span attributes are captured as data attributes on the Sentry span
        cx.span().set_attribute(KeyValue::new("my", "attribute"));

        // Captures a Sentry error message and associates it with the ongoing child span
        sentry::capture_message("Everything is on fire!", sentry::Level::Error);
    });
});
```

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
