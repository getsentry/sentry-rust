<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-anyhow

Adds support for capturing Sentry errors from [`anyhow::Error`].

This integration adds a new event *source*, which allows you to create events directly
from an [`anyhow::Error`] struct.  As it is only an event source it only needs to be
enabled using the `anyhow` cargo feature, it does not need to be enabled in the call to
[`sentry::init`](https://docs.rs/sentry/*/sentry/fn.init.html).

This integration does not need to be installed, instead it provides an extra function to
capture [`anyhow::Error`], optionally exposing it as a method on the
[`sentry::Hub`](https://docs.rs/sentry/*/sentry/struct.Hub.html) using the
[`AnyhowHubExt`] trait.

Like a plain [`std::error::Error`] being captured, [`anyhow::Error`] is captured with a
chain of all error sources, if present.  See
[`sentry::capture_error`](https://docs.rs/sentry/*/sentry/fn.capture_error.html) for
details of this.

## Example

```rust
use sentry_anyhow::capture_anyhow;

fn function_that_might_fail() -> anyhow::Result<()> {
    Err(anyhow::anyhow!("some kind of error"))
}

if let Err(err) = function_that_might_fail() {
    capture_anyhow(&err);
}
```

## Features

The `backtrace` feature will enable the corresponding feature in anyhow and allow you to
capture backtraces with your events.  It is enabled by default.

[`anyhow::Error`]: https://docs.rs/anyhow/*/anyhow/struct.Error.html

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
