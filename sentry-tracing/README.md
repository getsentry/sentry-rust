<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-tracing

Adds support for automatic Breadcrumb, Event and Transaction capturing from
tracing events, similar to the `sentry-log` crate.

The `tracing` crate is supported in three ways. First, events can be captured
as breadcrumbs for later. Secondly, error events can be captured as events
to Sentry. Finally, spans can be recorded as structured transaction events.
By default, events above `Info` are recorded as breadcrumbs, events above
`Error` are captured as error events, and spans above `Info` are recorded
as transactions.

By using this crate in combination with `tracing-subscriber` and its `log`
integration, `sentry-log` does not need to be used, as logs will be ingested
in the tracing system and generate events, thus be relayed to this crate. It
effectively replaces `sentry-log` when tracing is used.

## Examples

```rust
use std::time::Duration;

use tokio::time::sleep;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() {
    let _guard = sentry::init(sentry::ClientOptions {
        // Set this a to lower value in production
        traces_sample_rate: 1.0,
        ..sentry::ClientOptions::default()
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(sentry_tracing::layer())
        .init();

    outer().await;
}

// Functions instrumented by tracing automatically report
// their span as transactions
#[tracing::instrument]
async fn outer() {
    tracing::info!("Generates a breadcrumb");

    for _ in 0..10 {
        inner().await;
    }

    tracing::error!("Generates an event");
}

#[tracing::instrument]
async fn inner() {
    // Also works, since log events are ingested by the tracing system
    log::info!("Generates a breadcrumb");

    sleep(Duration::from_millis(100)).await;

    log::error!("Generates an event");
}
```

Or one might also set an explicit filter, to customize how to treat log
records:

```rust
use sentry_tracing::EventFilter;
use tracing_subscriber::prelude::*;

let layer = sentry_tracing::layer().event_filter(|md| match md.level() {
    &tracing::Level::ERROR => EventFilter::Event,
    _ => EventFilter::Ignore,
});

tracing_subscriber::registry().with(layer).init();
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
