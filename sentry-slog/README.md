<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-slog

Sentry `slog` Integration.

This mainly provides the [`SentryDrain`], which wraps another [`slog::Drain`]
and can be configured to forward [`slog::Record`]s to Sentry.
The [`SentryDrain`] can be used to create a `slog::Logger`.

*NOTE*: This integration currently does not process any `slog::KV` pairs,
but support for this will be added in the future.

## Examples

```rust
use sentry::{init, ClientOptions};
use sentry_slog::SentryDrain;

let _sentry = sentry::init(());

let drain = SentryDrain::new(slog::Discard);
let root = slog::Logger::root(drain, slog::o!());

slog::info!(root, "recorded as breadcrumb");
slog::warn!(root, "recorded as regular event");

assert_eq!(
    captured_event.breadcrumbs.as_ref()[0].message.as_deref(),
    Some("recorded as breadcrumb")
);
assert_eq!(
    captured_event.message.as_deref(),
    Some("recorded as regular event")
);

slog::crit!(root, "recorded as exception event");

assert_eq!(captured_event.exception.len(), 1);
```

The Drain can also be customized with a `filter`, and a `mapper`:

```rust
use sentry_slog::{exception_from_record, LevelFilter, RecordMapping, SentryDrain};

let drain = SentryDrain::new(slog::Discard)
    .filter(|level| match level {
        slog::Level::Critical | slog::Level::Error => LevelFilter::Event,
        _ => LevelFilter::Ignore,
    })
    .mapper(|record, kv| match record.level() {
        slog::Level::Critical | slog::Level::Error => {
            RecordMapping::Event(exception_from_record(record, kv))
        }
        _ => RecordMapping::Ignore,
    });
```

When a `mapper` is specified, a corresponding `filter` should also be
provided.

[`SentryDrain`]: https://docs.rs/sentry-slog/0.21.0/sentry_slog/struct.SentryDrain.html

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
