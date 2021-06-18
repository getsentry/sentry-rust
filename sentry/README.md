<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry

This crate provides support for logging events and errors / panics to the
[Sentry] error logging service. It integrates with the standard panic
system in Rust as well as a few popular error handling setups.

[Sentry]: https://sentry.io/

## Quickstart

The most convenient way to use this library is via the [`sentry::init`] function,
which starts a sentry client with a default set of integrations, and binds
it to the current [`Hub`].

The [`sentry::init`] function returns a guard that when dropped will flush Events that were not
yet sent to the sentry service. It has a two second deadline for this so shutdown of
applications might slightly delay as a result of this. Keep the guard around or sending events
will not work.

```rust
let _guard = sentry::init("https://key@sentry.io/42");
sentry::capture_message("Hello World!", sentry::Level::Info);
// when the guard goes out of scope here, the client will wait up to two
// seconds to send remaining events to the service.
```

[`sentry::init`]: https://docs.rs/sentry/0.21.0/sentry/fn.init.html
[`Hub`]: https://docs.rs/sentry/0.21.0/sentry/struct.Hub.html

## Integrations

What makes this crate useful are its various integrations. Some of them are enabled by 
default; See [Features]. Uncommon integrations or integrations for deprecated parts of 
the ecosystem require a feature flag. For available integrations and how to use them, see
[integrations](https://docs.rs/sentry/0.21.0/sentry/integrations/index.html) and
[apply_defaults](https://docs.rs/sentry/0.21.0/sentry/fn.apply_defaults.html). 

## Minimal API

This crate comes fully-featured. If the goal is to instrument libraries for usage
with sentry, or to extend sentry with a custom [`Integration`] or a [`Transport`],
one should use the [`sentry-core`] crate instead.

[`Integration`]: https://docs.rs/sentry/0.21.0/sentry/trait.Integration.html
[`Transport`]: https://docs.rs/sentry/0.21.0/sentry/trait.Transport.html
[`sentry-core`]: https://crates.io/crates/sentry-core

## Features

Additional functionality and integrations are enabled via feature flags. Some features require
extra setup to function properly. 

| Feature | Default | Is Integration | Deprecated | Additional notes |
| --- | --- | --- | --- | --- |
| `backtrace` | ✅ | 🔌 | | |
| `contexts` | ✅ | 🔌 | | |
| `panic` | ✅ | 🔌 | | |
| `transport` | ✅ | | | |
| `anyhow` | | 🔌 | | |
| `test` | | | | |
| `debug-images` | | 🔌 | | |
| `log` | | 🔌 |  |Requires additional setup; See [`sentry-log`]'s documentation. |
| `debug-logs` | | | ✂️ | | Requires additional setup; See [`sentry-log`]'s documentation. |
| `slog` | | 🔌 | | | Requires additional setup; See [`sentry-slog`]'s documentation. |
| `reqwest` | ✅ | | | |
| `native-tls` | ✅ | | | `reqwest` must be enabled. | |
| `rustls` | | | | `reqwest` must be enabled. `native-tls` must be disabled via `default-features = false`. | |
| `curl` | | | | |
| `surf` | | | | |

[`sentry-log`]: https://docs.rs/sentry-log
[`sentry-slog`]: https://docs.rs/sentry-slog

Descriptions of all features, grouped by an arbitrary set of categories, sorted in a similarly 
arbitrary order:

### Default features
- `backtrace`: Enables backtrace support.
- `contexts`: Enables capturing device, OS, and Rust contexts.
- `panic`: Enables support for capturing panics.
- `transport`: Enables the default transport, which is currently `reqwest` with `native-tls`.

### Debugging/Testing
- `anyhow`: Enables support for the `anyhow` crate.
- `test`: Enables testing support.
- `debug-images`: Attaches a list of loaded libraries to events (currently only supported on Unix).

### Logging
- `log`: Enables support for the `log` crate.
- `slog`: Enables support for the `slog` crate.
- `debug-logs`: **Deprecated**. Uses the `log` crate for internal logging.

### Transports
- `reqwest`: **Default**. Enables the `reqwest` transport.
- `native-tls`: **Default**. Uses the `native-tls` crate. This only affects the `reqwest` transport.
- `rustls`: Enables `rustls` support for `reqwest`. Please note that `native-tls` is a default 
  feature, and `default-features = false` must be set to completely disable building `native-tls` 
  dependencies.
- `curl`: Enables the curl transport.
- `surf`: Enables the surf transport.

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
