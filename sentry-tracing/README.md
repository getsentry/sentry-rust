<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-tracing

Capture `tracing` spans and events as Sentry traces and structured logs.

For most applications, we recommend installing only these two layers:

- [`span_layer`] captures `tracing` spans as Sentry transactions and spans.
- [`log_layer`] captures `tracing` events as
  [Sentry Logs](https://docs.sentry.io/product/explore/logs/). Requires the `logs` feature.

We also provide the following layers, but recommend the span and log layers above for most
applications:

- [`error_layer`] captures `tracing` events as Sentry error events, with support for tags and
  exception information.
- [`breadcrumb_layer`] adds `tracing` events as breadcrumbs on subsequent Sentry error events.

For migration from the legacy combined layer, see [`SentryLayer`].

## Getting started

Enable the `tracing` and `logs` features on `sentry` to use its tracing integration re-export.
Alternatively, enable `logs` on a direct `sentry-tracing` dependency and omit the alias import
below.
Set a traces sample rate to capture transactions, then install the span and log layers with
explicit level filters:

```rust
use sentry::integrations::tracing as sentry_tracing;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;

let _guard = sentry::init(
    sentry::ClientOptions::new()
        // Use a lower sample rate in production.
        .traces_sample_rate(1.0),
);

tracing_subscriber::registry()
    .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
    .with(sentry_tracing::log_layer().with_filter(LevelFilter::INFO))
    .init();
```

The layers do not filter by level internally. The filters above capture `INFO`, `WARN`, and
`ERROR` telemetry, excluding `DEBUG` and `TRACE`. Adjust each layer's filter independently to
control volume. Global subscriber filters also apply and can suppress telemetry before the
Sentry layers receive it.

## Spans

`span_layer()` creates a Sentry transaction when no Sentry span is active, or a child span
otherwise. For example,
[`tracing::instrument`](https://docs.rs/tracing/latest/tracing/attr.instrument.html) creates
spans for instrumented functions; arguments become span data unless skipped.

With the log layer installed, the `info!` event below produces a log associated with the same
trace as the instrumented functions:

```rust
#[tracing::instrument]
async fn outer() {
    inner(42).await;
}

#[tracing::instrument]
async fn inner(value: i32) {
    tracing::info!(value, "completed step");
}
```

By default, the Sentry span name matches the `tracing` span name, and its operation is
`<target>::<span name>`.

### Special span fields

- `sentry.name` overrides the Sentry span name, including when recorded after creation.
- `sentry.op` overrides the Sentry span operation, including when recorded after creation.
- `sentry.trace` continues the trace from a `sentry-trace` header. It must be set when a
  **root Sentry span** is created, meaning no Sentry span is active in the current scope.
  Applying it to a child Sentry span or recording it after creation has no effect.

```rust
#[tracing::instrument(skip_all, fields(
    sentry.name = "GET /payments",
    sentry.op = "http.server",
    sentry.trace = headers.get("sentry-trace").unwrap_or(&"".to_owned()),
))]
async fn handle_request(headers: std::collections::HashMap<String, String>) {
    // ...
}
```

## Logs

`log_layer()` sends events accepted by its filter as structured logs. Event fields become
searchable log attributes; fields with dots appear nested under their common prefix in the
Sentry Logs explorer.

```rust
tracing::info!(number = 42, my.key = "value", "Processed request");
```

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@sentry](https://x.com/sentry) on X for updates.
