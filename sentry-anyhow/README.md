<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-anyhow

Adds support for capturing Sentry errors from `anyhow::Error`.

## Example

```rust
use sentry_anyhow::capture_anyhow;
let result = match function_that_might_fail() {
    Ok(result) => result,
    Err(err) => {
        capture_anyhow(&err);
        return Err(err);
    }
};
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
