use std::time::{Duration, SystemTime};

/// The time until which a rate limit is active.
///
/// Updates only ever extend the timeout, so a shorter rate limit can never replace a longer
/// one, as the [rate limiting spec] requires.
///
/// [rate limiting spec]: https://develop.sentry.dev/sdk/foundations/transport/rate-limiting/#stage-2-determine-rate-limits
#[derive(Debug, Default)]
pub(super) struct RateLimitTimeout(Option<SystemTime>);

impl RateLimitTimeout {
    /// Extends the timeout to `until`, unless the timeout already ends later.
    pub fn update(&mut self, until: SystemTime) {
        self.0 = self.0.max(Some(until));
    }

    /// Returns the time left until the timeout ends, or [`None`] if it is not active.
    pub fn remaining(&self) -> Option<Duration> {
        self.0?.duration_since(SystemTime::now()).ok()
    }
}
