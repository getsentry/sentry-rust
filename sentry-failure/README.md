# sentry-failure

Adds support for capturing Sentry errors from `failure` types.

Failure errors and `Fail` objects can be logged with the failure integration.
This works really well if you use the `failure::Error` type or if you have
`failure::Fail` objects that use the failure context internally to gain a
backtrace.

## Example

```rust
use sentry_failure::capture_error;
let result = match function_that_might_fail() {
    Ok(result) => result,
    Err(err) => {
        capture_error(&err);
        return Err(err);
    }
};
```

To capture fails and not errors use `capture_fail`.

License: Apache-2.0
