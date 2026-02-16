<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-debug-images

The Sentry Debug Images integration.

The [`DebugImagesIntegration`] adds metadata about the loaded shared
libraries to Sentry [`Event`]s.

## Configuration

The integration by default attaches this information to all [`Event`]s, but
a custom filter can be defined as well.

```rust
use sentry_core::Level;
let integration = sentry_debug_images::DebugImagesIntegration::new()
    .filter(|event| event.level >= Level::Warning);
```

[`Event`]: https://docs.rs/sentry-debug-images/0.46.2/sentry_debug_images/sentry_core::protocol::Event

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@sentry](https://x.com/sentry) on X for updates.
