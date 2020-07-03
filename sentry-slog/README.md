# sentry-slog

Sentry `slog` Integration.

The sentry `slog` integration consists of two parts, the
[`SlogIntegration`] which configures how sentry should treat
`slog::Record`s, and the [`SentryDrain`], which can be used to create a
`slog::Logger`.

*NOTE*: This integration currently does not process any `slog::KV` pairs,
but support for this will be added in the future.

## Examples

```rust
use sentry::{init, ClientOptions};
use sentry_slog::{SentryDrain, SlogIntegration};

let integration = SlogIntegration::default();
let options = ClientOptions::default().add_integration(integration);
let _sentry = sentry::init(options);

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

The integration can also be customized with a `filter`, and a `mapper`:

```rust
use sentry_slog::{exception_from_record, LevelFilter, RecordMapping, SlogIntegration};

let integration = SlogIntegration::default()
    .filter(|level| match level {
        slog::Level::Critical | slog::Level::Error => LevelFilter::Event,
        _ => LevelFilter::Ignore,
    })
    .mapper(|record, kv| RecordMapping::Event(exception_from_record(record, kv)));
```

Please not that the `mapper` can override any classification from the
previous `filter`.

[`SlogIntegration`]: struct.SlogIntegration.html
[`SentryDrain`]: struct.SentryDrain.html

License: Apache-2.0
