//! Utilities reused across dependant crates and integrations.

const SENSITIVE_HEADERS_UPPERCASE: &[&str] = &[
    "AUTHORIZATION",
    "PROXY_AUTHORIZATION",
    "COOKIE",
    "SET_COOKIE",
    "X_FORWARDED_FOR",
    "X_REAL_IP",
    "X_API_KEY",
];

/// Determines if the HTTP header with the given name shall be considered as potentially carrying
/// sensitive data.
pub fn is_sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADERS_UPPERCASE.contains(&name.to_ascii_uppercase().replace("-", "_").as_str())
}
