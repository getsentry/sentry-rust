<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-log

Adds support for automatic Breadcrumb, Event, and Log capturing from `log` records.

The `log` crate is supported in three ways:
- Records can be captured as Sentry events. These are grouped and show up in the Sentry
  [issues](https://docs.sentry.io/product/issues/) page, representing high severity issues to be
  acted upon.
- Records can be captured as [breadcrumbs](https://docs.sentry.io/product/issues/issue-details/breadcrumbs/).
  Breadcrumbs create a trail of what happened prior to an event, and are therefore sent only when
  an event is captured, either manually through e.g. `sentry::capture_message` or through integrations
  (e.g. the panic integration is enabled (default) and a panic happens).
- Records can be captured as traditional [logs](https://docs.sentry.io/product/explore/logs/)
  Logs can be viewed and queried in the Logs explorer.

By default anything at or above `Info` is recorded as a breadcrumb and
anything at or above `Error` is captured as error event.
Additionally, if the `sentry` crate is used with the `logs` feature flag, anything at or above `Info`
is captured as a [Structured Log](https://docs.sentry.io/product/explore/logs/).

## Examples

```rust
let mut log_builder = pretty_env_logger::formatted_builder();
log_builder.parse_filters("info");
let logger = sentry_log::SentryLogger::with_dest(log_builder.build());

log::set_boxed_logger(Box::new(logger)).unwrap();
log::set_max_level(log::LevelFilter::Info);

let _sentry = sentry::init(());

log::info!("Generates a breadcrumb");
log::error!("Generates an event");
```

Or one might also set an explicit filter, to customize how to treat log
records:

```rust
use sentry_log::LogFilter;

let logger = sentry_log::SentryLogger::new().filter(|md| match md.level() {
    log::Level::Error => LogFilter::Event,
    _ => LogFilter::Ignore,
});
```

## Sending multiple items to Sentry

To map a log record to multiple items in Sentry, you can combine multiple log filters
using the bitwise or operator:

```rust
use sentry_log::LogFilter;

let logger = sentry_log::SentryLogger::new().filter(|md| match md.level() {
    log::Level::Error => LogFilter::Event,
    log::Level::Warn => LogFilter::Breadcrumb | LogFilter::Log,
    _ => LogFilter::Ignore,
});
```

If you're using a custom record mapper instead of a filter, you can return a `Vec<RecordMapping>`
from your mapper function to send multiple items to Sentry from a single log record:

```rust
use sentry_log::{RecordMapping, SentryLogger, event_from_record, breadcrumb_from_record};

let logger = SentryLogger::new().mapper(|record| {
    match record.level() {
        log::Level::Error => {
            // Send both an event and a breadcrumb for errors
            vec![
                RecordMapping::Event(event_from_record(record)),
                RecordMapping::Breadcrumb(breadcrumb_from_record(record)),
            ]
        }
        log::Level::Warn => RecordMapping::Breadcrumb(breadcrumb_from_record(record)).into(),
        _ => RecordMapping::Ignore.into(),
    }
});
```

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@sentry](https://x.com/sentry) on X for updates.
