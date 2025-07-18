//! Utilities reused across dependant crates and integrations.

use url::Url;

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
pub fn scrub_pii_from_url(url: &mut Url) {
    if !url.username().is_empty() {
        let _ = url.set_username(PII_REPLACEMENT);
    }
    if url.password().is_some() {
        let _ = url.set_password(Some(PII_REPLACEMENT));
    }
}
