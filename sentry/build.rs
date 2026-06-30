fn main() {
    // Define cfg aliases for better readability, and to reduce repetition.
    cfg_aliases::cfg_aliases! {
        sentry_embedded_svc_http: { all(target_os = "espidf", feature = "embedded-svc-http") },
        sentry_any_http_transport: { any(
            feature = "reqwest",
            feature = "curl",
            feature = "ureq",
            sentry_embedded_svc_http
        )},
    }
}
