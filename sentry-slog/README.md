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

The integration also supports [`slog::KV`] pairs. They will be added to the
breadcrumb `data` or the event `extra` properties respectively.

## Examples

```rust
use sentry_slog::SentryDrain;

let _sentry = sentry::init(());

let drain = SentryDrain::new(slog::Discard);
let root = slog::Logger::root(drain, slog::o!("global_kv" => 1234));

slog::info!(root, "recorded as breadcrumb"; "breadcrumb_kv" => Some("breadcrumb"));
slog::warn!(root, "recorded as regular event"; "event_kv" => "event");

let breadcrumb = &captured_event.breadcrumbs.as_ref()[0];
assert_eq!(
    breadcrumb.message.as_deref(),
    Some("recorded as breadcrumb")
);
assert_eq!(breadcrumb.data["breadcrumb_kv"], "breadcrumb");
assert_eq!(breadcrumb.data["global_kv"], 1234);

assert_eq!(
    captured_event.message.as_deref(),
    Some("recorded as regular event")
);
assert_eq!(captured_event.extra["event_kv"], "event");
assert_eq!(captured_event.extra["global_kv"], 1234);

slog::crit!(root, "recorded as exception event");

assert_eq!(
    captured_event.message.as_deref(),
    Some("recorded as exception event")
);
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

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
