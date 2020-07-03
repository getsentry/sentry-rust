# sentry-anyhow

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

License: Apache-2.0
