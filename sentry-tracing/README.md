<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-tracing

Adds support for automatic Breadcrumb and Event capturing from tracing events,
similar to the `sentry-log` crate.

The `tracing` crate is supported in two ways. First, events can be captured as
breadcrumbs for later. Secondly, error events can be captured as events to
Sentry. By default, anything above `Info` is recorded as breadcrumb and
anything above `Error` is captured as error event.

By using this crate in combination with `tracing-subscriber` and its `log`
integration, `sentry-log` does not need to be used, as logs will be ingested
in the tracing system and generate events, thus be relayed to this crate. It
effectively replaces `sentry-log` when tracing is used.

## Examples

```rust
use tracing_subscriber::prelude::*;

tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer())
    .with(sentry_tracing::layer())
    .try_init()
    .unwrap();

let _sentry = sentry::init(());

tracing::info!("Generates a breadcrumb");
tracing::error!("Generates an event");
// Also works, since log events are ingested by the tracing system
log::info!("Generates a breadcrumb");
log::error!("Generates an event");
```

Or one might also set an explicit filter, to customize how to treat log
records:

```rust
use sentry_tracing::EventFilter;

let layer = sentry_tracing::layer().filter(|md| match md.level() {
    &tracing::Level::ERROR => EventFilter::Event,
    _ => EventFilter::Ignore,
});
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates

