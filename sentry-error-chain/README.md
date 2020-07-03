# sentry-error-chain

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

License: Apache-2.0
