<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-panic

The Sentry Panic handler Integration.

The `PanicIntegration`, which is enabled by default in `sentry`, installs a
panic handler that will automatically dispatch all errors to Sentry that
are caused by a panic.
Additionally, panics are forwarded to the previously registered panic hook.

## Configuration

The panic integration can be configured with an additional extractor, which
might optionally create a sentry `Event` out of a `PanicInfo`.

```rust
let integration = sentry_panic::PanicIntegration::default().add_extractor(|info| None);
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
