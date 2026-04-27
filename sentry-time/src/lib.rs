use std::time::SystemTime;

/// Returns the current wall-clock time as a [`std::time::SystemTime`], sourced from
/// [`chrono::Utc::now`] so it works on `wasm32-unknown-unknown` (where
/// [`std::time::SystemTime::now`] panics).
pub fn now_system_time() -> SystemTime {
    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let now = chrono::Utc::now();
        let secs = now.timestamp() as u64;
        let nanos = now.timestamp_subsec_nanos();
        SystemTime::UNIX_EPOCH + std::time::Duration::new(secs, nanos)
    }
}
