<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-debug-images

The Sentry Debug Images Integration.

The `DebugImagesIntegration` adds metadata about the loaded shared libraries
to Sentry `Event`s.

## Configuration

The integration by default attaches this information to all Events, but a
custom filter can be defined as well.

```rust
use sentry_core::Level;
let integration = sentry_debug_images::DebugImagesIntegration {
    filter: Box::new(|event| event.level >= Level::Warning),
    ..Default::default()
};
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
