<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <picture>
      <source srcset="https://sentry-brand.storage.googleapis.com/sentry-logo-white.png" media="(prefers-color-scheme: dark)" />
      <source srcset="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" media="(prefers-color-scheme: light), (prefers-color-scheme: no-preference)" />
      <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" alt="Sentry" width="280">
    </picture>
  </a>
</p>

# Sentry Rust SDK: sentry-debug-images

The Sentry Debug Images integration.

The [`DebugImagesIntegration`] adds metadata about the loaded shared
libraries to Sentry [`Event`]s.

This Integration only works on Unix-like OSes right now. Support for Windows
will be added in the future.

## Configuration

The integration by default attaches this information to all [`Event`]s, but
a custom filter can be defined as well.

```rust
use sentry_core::Level;
let integration = sentry_debug_images::DebugImagesIntegration::new()
    .filter(|event| event.level >= Level::Warning);
```

[`Event`]: https://docs.rs/sentry-debug-images/0.25.0/sentry_debug-images/sentry_core::protocol::Event

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
