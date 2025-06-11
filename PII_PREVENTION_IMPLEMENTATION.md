# PII Prevention Implementation for Sentry HTTP Integrations

## Overview

This document outlines the implementation of PII (Personally Identifiable Information) prevention for HTTP requests in the `sentry-actix` and `sentry-tower` integrations, addressing issue #516 from the sentry-rust repository.

## Problem

HTTP integrations were capturing full URLs including query parameters and fragments, which could contain sensitive data like passwords, tokens, or user IDs. According to the Sentry specification, URLs should be stripped of these components and stored separately to prevent PII leaks.

## Sentry Specification Requirements

Based on the [Sentry Developer Documentation](https://develop.sentry.dev/sdk/expected-features/data-handling/), the requirements are:

1. **Main URL field**: Should contain only the base URL without query parameters or fragments
2. **Query parameters**: Should be stored separately in the `query_string` field for HTTP requests
3. **Fragments**: Should be stored separately in the `env["http.fragment"]` field for HTTP requests  
4. **Authority filtering**: Userinfo (username:password) should be filtered regardless of `sendDefaultPii` setting

## Implementation

### 1. Shared Function in `sentry-core/src/utils.rs`

Created a shared `strip_url_for_privacy()` function with the following features:
- Strips query parameters and fragments from URLs
- Filters userinfo (username:password) from authority with `[Filtered]:[Filtered]` placeholders
- Returns the stripped URL along with separate query and fragment strings
- Includes comprehensive test coverage

```rust
pub fn strip_url_for_privacy(mut url: Url) -> (Url, Option<String>, Option<String>) {
    // Implementation details...
}
```

### 2. Updated `sentry-actix` Integration

- Modified `sentry_request_from_http()` function to use the shared URL stripping function
- Query parameters stored in `request.query_string` field
- Fragments stored in `request.env["http.fragment"]` field
- Added comprehensive test to verify PII prevention

### 3. Updated `sentry-tower` Integration  

- Modified the `Service::call()` method to use the shared URL stripping function
- Same storage approach as sentry-actix for consistency
- Added test with proper feature flags (`http` feature required)

### 4. Specification Compliance

The implementation ensures compliance with Sentry's PII prevention specification:

✅ **URL Stripping**: Main URL field contains only base URL without sensitive parameters
✅ **Query Storage**: Query parameters stored separately in `query_string` field  
✅ **Fragment Storage**: Fragments stored separately in `env["http.fragment"]` field
✅ **Authority Filtering**: Userinfo filtered regardless of PII settings
✅ **Backward Compatibility**: All existing functionality preserved

## Testing

### Test Coverage

1. **Core Function Tests** (`sentry-core`):
   - Basic URL stripping with query and fragment
   - Userinfo filtering in authority
   - URLs without query/fragment parameters
   - Username-only scenarios

2. **Integration Tests**:
   - `sentry-actix`: HTTP requests with sensitive query parameters
   - `sentry-tower`: HTTP requests with sensitive query parameters (requires `--features http`)

### Running Tests

```bash
# Core functionality tests
cargo test --package sentry-core test_strip_url_for_privacy

# sentry-actix integration test  
cargo test --package sentry-actix test_url_stripping_for_pii_prevention

# sentry-tower integration test (requires http feature)
cargo test --package sentry-tower --features http test_url_stripping_for_pii_prevention
```

## Key Benefits

1. **Security Enhancement**: Prevents accidental leakage of sensitive data in URLs
2. **Specification Compliance**: Follows Sentry's official PII prevention guidelines
3. **Code Reuse**: Shared implementation reduces duplication
4. **Comprehensive Testing**: Ensures reliability and correctness
5. **Backward Compatibility**: No breaking changes to existing APIs

## Example

Before:
```
URL: https://api.example.com/users/123?password=secret&token=abc123
```

After:
```
URL: https://api.example.com/users/123
Query String: password=secret&token=abc123  
```

This ensures that sensitive data is stored separately and can be properly filtered by Sentry's server-side scrubbing mechanisms.