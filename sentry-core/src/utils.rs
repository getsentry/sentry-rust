//! Utilities reused across dependant crates and integrations.

use std::time::{Duration, SystemTime};

/// Returns the current wall-clock time as a [`std::time::SystemTime`], sourced from
/// [`chrono::Utc::now`] so it works on `wasm32-unknown-unknown` (where
/// [`std::time::SystemTime::now`] panics).
pub fn now_system_time() -> std::time::SystemTime {
    let now = chrono::Utc::now();
    let secs = now.timestamp() as u64;
    let nanos = now.timestamp_subsec_nanos();
    SystemTime::UNIX_EPOCH + Duration::new(secs, nanos)
}

const SENSITIVE_HEADERS_UPPERCASE: &[&str] = &[
    "AUTHORIZATION",
    "PROXY_AUTHORIZATION",
    "COOKIE",
    "SET_COOKIE",
    "X_FORWARDED_FOR",
    "X_REAL_IP",
    "X_API_KEY",
];

const PII_REPLACEMENT: &str = "[Filtered]";

/// Determines if the HTTP header with the given name shall be considered as potentially carrying
/// sensitive data.
pub fn is_sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADERS_UPPERCASE.contains(&name.to_ascii_uppercase().replace("-", "_").as_str())
}

/// Scrub PII (username and password) from the given URL.
pub fn scrub_pii_from_url(mut url: url::Url) -> url::Url {
    // the set calls will fail and return an error if the URL is relative
    // in those cases, just ignore the errors
    if !url.username().is_empty() {
        let _ = url.set_username(PII_REPLACEMENT);
    }
    if url.password().is_some() {
        let _ = url.set_password(Some(PII_REPLACEMENT));
    }
    url
}
