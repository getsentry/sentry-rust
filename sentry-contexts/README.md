<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <picture>
      <source srcset="https://sentry-brand.storage.googleapis.com/sentry-logo-white.png" media="(prefers-color-scheme: dark)" />
      <source srcset="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" media="(prefers-color-scheme: light), (prefers-color-scheme: no-preference)" />
      <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" alt="Sentry" width="280">
    </picture>
  </a>
</p>

# Sentry Rust SDK: sentry-contexts

Adds Contexts to Sentry Events.

This integration is enabled by default in `sentry` and adds `device`, `os`
and `rust` contexts to Events, and also sets a `server_name` if it is not
already defined.

See the [Contexts Interface] documentation for more info.

## Examples

```rust
let integration = sentry_contexts::ContextIntegration::new().add_os(false);
let _sentry = sentry::init(sentry::ClientOptions::new().add_integration(integration));
```

[Contexts Interface]: https://develop.sentry.dev/sdk/event-payloads/contexts/

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
