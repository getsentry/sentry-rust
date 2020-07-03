# sentry

<p style="margin: -10px 0 0 15px; padding: 0; float: right;">
  <a href="https://sentry.io/"><img
    src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png"
    style="width: 260px"></a>
</p>

This crate provides support for logging events and errors / panics to the
[Sentry](https://sentry.io/) error logging service.  It integrates with the standard panic
system in Rust as well as a few popular error handling setups.

## Quickstart

The most convenient way to use this library is the [`sentry::init`] function,
which starts a sentry client with a default set of integrations, and binds
it to the current [`Hub`].

The [`sentry::init`] function returns a guard that when dropped will flush Events that were not
yet sent to the sentry service.  It has a two second deadline for this so shutdown of
applications might slightly delay as a result of this.  Keep the guard around or sending events
will not work.

```rust
let _guard = sentry::init("https://key@sentry.io/42");
sentry::capture_message("Hello World!", sentry::Level::Info);
// when the guard goes out of scope here, the client will wait up to two
// seconds to send remaining events to the service.
```

[`sentry::init`]: fn.init.html
[`Hub`]: struct.Hub.html

## Integrations

What makes this crate useful are the various integrations that exist.  Some of them are enabled
by default, some uncommon ones or for deprecated parts of the ecosystem a feature flag needs to
be enabled.  For the available integrations and how to use them see
[integrations](integrations/index.html) and [apply_defaults](fn.apply_defaults.html).

## Minimal API

This crate comes fully featured. If the goal is to instrument libraries for usage
with sentry, or to extend sentry with a custom [`Integration`] or a [`Transport`],
one should use the [`sentry-core`] crate instead.

[`Integration`]: trait.Integration.html
[`Transport`]: trait.Transport.html
[`sentry-core`]: https://crates.io/crates/sentry-core

## Features

Functionality of the crate can be turned on and off by feature flags.  This is the current list
of feature flags:

Default features:

* `backtrace`: Enables backtrace support.
* `contexts`: Enables capturing device, os, and rust contexts.
* `failure`: Enables support for the `failure` crate.
* `panic`: Enables support for capturing panics.
* `transport`: Enables the default transport, which is currently `reqwest` with `native-tls`.

Additional features:

* `anyhow`: Enables support for the `anyhow` crate.
* `debug-images`: Attaches a list of loaded libraries to events (currently only supported on unix).
* `error-chain`: Enables support for the `error-chain` crate.
* `log`: Enables support for the `log` crate.
* `slog`: Enables support for the `slog` crate.
* `test`: Enables testing support.
* `debug-logs`: Uses the `log` crate for internal logging.
* `reqwest`: Enables the `reqwest` transport, which is currently the default.
* `curl`: Enables the curl transport.
* `surf`: Enables the surf transport.
* `native-tls`: Uses the `native-tls` crate, which is currently the default.
  This only has an effect on the `reqwest` transport.
* `rustls`: Enables the `rustls` support of the `reqwest` transport.

License: Apache-2.0
