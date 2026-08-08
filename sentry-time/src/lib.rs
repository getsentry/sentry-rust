use std::time::SystemTime;

pub use web_time::Instant;

/// Returns the current wall-clock time as a [`std::time::SystemTime`], sourced from
/// [`web_time::SystemTime`] so it works on `wasm32-unknown-unknown` (where
/// [`std::time::SystemTime::now`] panics).
pub fn now_system_time() -> SystemTime {
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        SystemTime::now()
    }

    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        use web_time::web::SystemTimeExt as _;
        web_time::SystemTime::now().to_std()
    }
}
