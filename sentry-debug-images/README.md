# sentry-debug-images

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

License: Apache-2.0
