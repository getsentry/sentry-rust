<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-error-chain

Adds support for the error-chain crate.

Errors created by the `error-chain` crate can be logged with the
`error_chain` integration.

## Example

```rust
use sentry_error_chain::{capture_error_chain, ErrorChainIntegration};
let _sentry =
    sentry::init(sentry::ClientOptions::default().add_integration(ErrorChainIntegration));
let result = match function_that_might_fail() {
    Ok(result) => result,
    Err(err) => {
        capture_error_chain(&err);
        return Err(err);
    }
};
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
