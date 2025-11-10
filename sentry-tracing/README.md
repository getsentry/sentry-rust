<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-tracing

Support for automatic breadcrumb, event, and trace capturing from `tracing` events and spans.

The `tracing` crate is supported in four ways:
- `tracing` events can be captured as Sentry events. These are grouped and show up in the Sentry
  [issues](https://docs.sentry.io/product/issues/) page, representing high severity issues to be
  acted upon.
- `tracing` events can be captured as [breadcrumbs](https://docs.sentry.io/product/issues/issue-details/breadcrumbs/).
  Breadcrumbs create a trail of what happened prior to an event, and are therefore sent only when
  an event is captured, either manually through e.g. `sentry::capture_message` or through integrations
  (e.g. the panic integration is enabled (default) and a panic happens).
- `tracing` events can be captured as traditional [structured logs](https://docs.sentry.io/product/explore/logs/).
  The `tracing` fields are captured as attributes on the logs, which can be queried in the Logs
  explorer. (Available on crate feature `logs`)
- `tracing` spans can be captured as Sentry spans. These can be used to provide more contextual
  information for errors, diagnose [performance
  issues](https://docs.sentry.io/product/insights/overview/), and capture additional attributes to
  aggregate and compute [metrics](https://docs.sentry.io/product/explore/trace-explorer/).

By default, events above `Info` are recorded as breadcrumbs, events above `Error` are captured
as error events, and spans above `Info` are recorded as spans.

## Configuration

To fully enable the tracing integration, set the traces sample rate and add a layer to the
tracing subscriber:

```rust
use tracing_subscriber::prelude::*;

let _guard = sentry::init(sentry::ClientOptions {
    // Enable capturing of traces; set this a to lower value in production:
    traces_sample_rate: 1.0,
    ..sentry::ClientOptions::default()
});

// Register the Sentry tracing layer to capture breadcrumbs, events, and spans:
tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer())
    .with(sentry::integrations::tracing::layer())
    .init();
```

You can customize the behavior of the layer by providing an explicit event filter, to customize which events
are captured by Sentry and the data type they are mapped to.
Similarly, you can provide a span filter to customize which spans are captured by Sentry.

```rust
use sentry::integrations::tracing::EventFilter;
use tracing_subscriber::prelude::*;

let sentry_layer = sentry::integrations::tracing::layer()
    .event_filter(|md| match *md.level() {
        tracing::Level::ERROR => EventFilter::Event,
        _ => EventFilter::Ignore,
    })
    .span_filter(|md| matches!(*md.level(), tracing::Level::ERROR | tracing::Level::WARN));

tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer())
    .with(sentry_layer)
    .init();
```

In addition, a custom event mapper can be provided, to fully customize if and how `tracing` events are converted to Sentry data.

Note that if both an event mapper and event filter are set, the mapper takes precedence, thus the
filter has no effect.

## Capturing breadcrumbs

Tracing events automatically create breadcrumbs that are attached to the current scope in
Sentry. They show up on errors and transactions captured within this scope as shown in the
examples below.

Fields passed to the event macro are automatically tracked as structured data in Sentry. For
breadcrumbs, they are shown directly with the breadcrumb message. For other types of data, read
below.

```rust
for i in 0..10 {
    tracing::debug!(number = i, "Generates a breadcrumb");
}
```

## Capturing logs

Tracing events can be captured as traditional structured logs in Sentry.
This is gated by the `logs` feature flag and requires setting up a custom Event filter/mapper
to capture logs. You also need to pass `enable_logs: true` in your `sentry::init` call.

```rust
// assuming `tracing::Level::INFO => EventFilter::Log` in your `event_filter`
for i in 0..10 {
    tracing::info!(number = i, my.key = "val", my.num = 42, "This is a log");
}
```

The fields of a `tracing` event are captured as attributes of the log.
Logs can be viewed and queried in the Logs explorer based on message and attributes.
Fields containing dots will be displayed as nested under their common prefix in the UI.

## Tracking Errors

The easiest way to emit errors is by logging an event with `ERROR` level. This will create a
grouped issue in Sentry. To add custom information, prepend the message with fields. It is also
possible to add Sentry tags if a field is prefixed with `"tags."`

```rust
tracing::error!(
    field = "value",                  // will become a context field
    tags.custom = "value",            // will become a tag in Sentry
    "this is an error with a custom tag",
);
```

To track [error structs](https://docs.rs/sentry-tracing/0.45.0/sentry_tracing/std::error::Error), assign a reference to error trait object as field
in one of the logging macros. By convention, it is recommended to use the `ERROR` level and
assign it to a field called `error`, although the integration will also work with all other
levels and field names.

All other fields passed to the macro are captured in a custom "Tracing Fields" context in
Sentry.

```rust
use std::error::Error;
use std::io;

let custom_error = io::Error::new(io::ErrorKind::Other, "oh no");
tracing::error!(error = &custom_error as &dyn Error);
```

It is also possible to combine error messages with error structs. In Sentry, this creates issues
grouped by the message and location of the error log, and adds the passed error as nested
source.

```rust
use std::error::Error;
use std::io;

let custom_error = io::Error::new(io::ErrorKind::Other, "oh no");
tracing::error!(error = &custom_error as &dyn Error, "my operation failed");
```

## Sending multiple items to Sentry

To map a `tracing` event to multiple items in Sentry, you can combine multiple event filters
using the bitwise or operator:

```rust
use sentry::integrations::tracing::EventFilter;
use tracing_subscriber::prelude::*;

let sentry_layer = sentry::integrations::tracing::layer()
    .event_filter(|md| match *md.level() {
        tracing::Level::ERROR => EventFilter::Event | EventFilter::Log,
        tracing::Level::TRACE => EventFilter::Ignore,
        _ => EventFilter::Log,
    })
    .span_filter(|md| matches!(*md.level(), tracing::Level::ERROR | tracing::Level::WARN));

tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer())
    .with(sentry_layer)
    .init();
```

If you're using a custom event mapper instead of an event filter, use `EventMapping::Combined`.

## Tracing Spans

The integration automatically tracks `tracing` spans as spans in Sentry. A convenient way to do
this is with the `#[instrument]` attribute macro, which creates a span/transaction for the function
in Sentry.

Function arguments are added as context fields automatically, which can be configured through
attribute arguments. Refer to documentation of the macro for more information.

```rust
use std::time::Duration;

use tracing_subscriber::prelude::*;

// Functions instrumented by tracing automatically
// create spans/transactions around their execution.
#[tracing::instrument]
async fn outer() {
    for i in 0..10 {
        inner(i).await;
    }
}

// This creates spans inside the outer transaction, unless called directly.
#[tracing::instrument]
async fn inner(i: u32) {
    // Also works, since log events are ingested by the tracing system
    tracing::debug!(number = i, "Generates a breadcrumb");

    tokio::time::sleep(Duration::from_millis(100)).await;
}
```

By default, the name of the span sent to Sentry matches the name of the `tracing` span, which
is the name of the function when using `tracing::instrument`, or the name passed to the
`tracing::<level>_span` macros.

By default, the `op` of the span sent to Sentry is `default`.

### Special Span Fields

Some fields on spans are treated specially by the Sentry tracing integration:
- `sentry.name`: overrides the span name sent to Sentry.
  This is useful to customize the span name when using `#[tracing::instrument]`, or to update
  it retroactively (using `span.record`) after the span has been created.
- `sentry.op`: overrides the span `op` sent to Sentry.
- `sentry.trace`: in Sentry, the `sentry-trace` header is sent with HTTP requests to achieve distributed tracing.
  If the value of this field is set to the value of a valid `sentry-trace` header, which
  other Sentry SDKs send automatically with outgoing requests, then the SDK will continue the trace using the given distributed tracing information.
  This is useful to achieve distributed tracing at service boundaries by using only the
  `tracing` API.
  Note that `sentry.trace` will only be effective on span creation (it cannot be applied retroactively)
  and requires the span it's applied to to be a root span, i.e. no span should active upon its
  creation.


Example:

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

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@sentry](https://x.com/sentry) on X for updates.
