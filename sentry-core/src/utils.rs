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

/// Determines if the HTTP header with the given name shall be considered as potentially carrying
/// sensitive data.
pub fn is_sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADERS_UPPERCASE.contains(&name.to_ascii_uppercase().replace("-", "_").as_str())
}

/// Strip query parameters and fragment from URL to prevent PII leaks.
/// Also filters out userinfo (username:password) from authority regardless of PII settings.
/// 
/// According to the Sentry specification:
/// - Query parameters should be stored separately to prevent PII leaks
/// - Fragments should be stored separately
/// - Authority with userinfo should be filtered out regardless of sendDefaultPii setting
/// 
/// Returns (stripped_url, query_string, fragment)
pub fn strip_url_for_privacy(mut url: Url) -> (Url, Option<String>, Option<String>) {
    let query = if url.query().is_some() {
        Some(url.query().unwrap().to_string())
    } else {
        None
    };
    
    let fragment = if url.fragment().is_some() {
        Some(url.fragment().unwrap().to_string())
    } else {
        None
    };
    
    // Filter out userinfo (username:password) from authority regardless of PII settings
    // According to spec: "If an authority is present in the URL (https://username:password@example.com), 
    // the authority must be replaced with a placeholder regardless of sendDefaultPii"
    if url.username() != "" || url.password().is_some() {
        // Replace userinfo with filtered placeholders
        let _ = url.set_username("[Filtered]");
        let _ = url.set_password(Some("[Filtered]"));
    }
    
    // Clear query and fragment to prevent PII leaks
    url.set_query(None);
    url.set_fragment(None);
    
    (url, query, fragment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_url_for_privacy_basic() {
        let url = "https://example.com/api/users/123?password=secret&token=abc123#section1"
            .parse::<Url>()
            .unwrap();
        
        let (stripped_url, query, fragment) = strip_url_for_privacy(url);
        
        // URL should be stripped of query and fragment
        let url_str = stripped_url.as_str();
        assert!(!url_str.contains("password"));
        assert!(!url_str.contains("token"));
        assert!(!url_str.contains("?"));
        assert!(!url_str.contains("#"));
        assert_eq!(url_str, "https://example.com/api/users/123");
        
        // Query should be stored separately
        assert_eq!(query, Some("password=secret&token=abc123".to_string()));
        
        // Fragment should be stored separately
        assert_eq!(fragment, Some("section1".to_string()));
    }

    #[test]
    fn test_strip_url_for_privacy_userinfo() {
        let url = "https://username:password@example.com/api/data?param=value"
            .parse::<Url>()
            .unwrap();
        
        let (stripped_url, query, fragment) = strip_url_for_privacy(url);
        
        // URL should have userinfo filtered according to spec
        let url_str = stripped_url.as_str();
        // Square brackets get URL encoded, so [Filtered] becomes %5BFiltered%5D
        assert!(url_str.contains("%5BFiltered%5D:%5BFiltered%5D@"));
        assert!(!url_str.contains("username"));
        assert!(!url_str.contains("password"));
        assert!(!url_str.contains("param=value"));
        assert_eq!(url_str, "https://%5BFiltered%5D:%5BFiltered%5D@example.com/api/data");
        
        // Query should be stored separately
        assert_eq!(query, Some("param=value".to_string()));
        
        // No fragment
        assert_eq!(fragment, None);
    }

    #[test]
    fn test_strip_url_for_privacy_no_query_or_fragment() {
        let url = "https://example.com/api/users/123"
            .parse::<Url>()
            .unwrap();
        
        let (stripped_url, query, fragment) = strip_url_for_privacy(url);
        
        // URL should remain the same
        assert_eq!(stripped_url.as_str(), "https://example.com/api/users/123");
        
        // No query or fragment
        assert_eq!(query, None);
        assert_eq!(fragment, None);
    }

    #[test]
    fn test_strip_url_for_privacy_username_only() {
        let url = "https://user@example.com/path"
            .parse::<Url>()
            .unwrap();
        
        let (stripped_url, _query, _fragment) = strip_url_for_privacy(url);
        
        // URL should have username filtered
        let url_str = stripped_url.as_str();
        // Square brackets get URL encoded
        assert!(url_str.contains("%5BFiltered%5D:%5BFiltered%5D@"));
        assert!(!url_str.contains("user"));
        assert_eq!(url_str, "https://%5BFiltered%5D:%5BFiltered%5D@example.com/path");
    }
}
