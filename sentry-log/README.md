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

By default anything above `Info` is recorded as a breadcrumb and
anything above `Error` is captured as error event.

To capture records as Sentry logs:
1. Enable the `logs` feature of the `sentry` crate.
2. Initialize the SDK with `enable_logs: true` in your client options.
3. Set up a custom filter (see below) to map records to logs (`LogFilter::Log`) based on criteria such as severity.

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

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
