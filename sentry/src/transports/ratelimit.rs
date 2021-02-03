use httpdate::parse_http_date;
use std::time::{Duration, SystemTime};

// TODO: maybe move this someplace where we can filter an `Envelope`s items.

/// A Utility that helps with rate limiting sentry requests.
#[derive(Debug, Default)]
pub struct RateLimiter {
    global: Option<SystemTime>,
    error: Option<SystemTime>,
    session: Option<SystemTime>,
    transaction: Option<SystemTime>,
}

impl RateLimiter {
    /// Create a new RateLimiter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the RateLimiter with information from a `Retry-After` header.
    pub fn update_from_retry_after(&mut self, header: &str) {
        let new_time = if let Ok(value) = header.parse::<f64>() {
            Some(SystemTime::now() + Duration::from_secs(value.ceil() as u64))
        } else if let Ok(value) = parse_http_date(header) {
            Some(value)
        } else {
            None
        };

        if new_time.is_some() {
            self.global = new_time;
        }
    }

    /// Updates the RateLimiter with information from a `X-Sentry-Rate-Limits` header.
    pub fn update_from_sentry_header(&mut self, header: &str) {
        // <rate-limit> = (<group>,)+
        // <group> = <time>:(<category>;)+:<scope>(:<reason>)?

        let mut parse_group = |group: &str| {
            let mut splits = group.split(':');
            let seconds = splits.next()?.parse::<f64>().ok()?;
            let categories = splits.next()?;
            let _scope = splits.next()?;

            let new_time = Some(SystemTime::now() + Duration::from_secs(seconds.ceil() as u64));

            if categories.is_empty() {
                self.global = new_time;
            }

            for category in categories.split(';') {
                match category {
                    "error" => self.error = new_time,
                    "session" => self.session = new_time,
                    "transaction" => self.transaction = new_time,
                    _ => {}
                }
            }
            Some(())
        };

        for group in header.split(',') {
            parse_group(group.trim());
        }
    }

    /// Query the RateLimiter for a certain category of event.
    pub fn is_disabled(&self, category: RateLimitingCategory) -> Option<Duration> {
        if let Some(ts) = self.global {
            let time_left = ts.duration_since(SystemTime::now()).ok();
            if time_left.is_some() {
                return time_left;
            }
        }
        let time_left = match category {
            RateLimitingCategory::Any => self.global,
            RateLimitingCategory::Error => self.error,
            RateLimitingCategory::Session => self.session,
            RateLimitingCategory::Transaction => self.transaction,
        }?;
        time_left.duration_since(SystemTime::now()).ok()
    }
}

/// The Category of payload that a Rate Limit refers to.
#[non_exhaustive]
#[allow(dead_code)]
pub enum RateLimitingCategory {
    /// Rate Limit for any kind of payload.
    Any,
    /// Rate Limit pertaining to Errors.
    Error,
    /// Rate Limit pertaining to Sessions.
    Session,
    /// Rate Limit pertaining to Transactions.
    Transaction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentry_header() {
        let mut rl = RateLimiter::new();
        rl.update_from_sentry_header("120:error:project:reason, 60:session:foo");

        assert!(rl.is_disabled(RateLimitingCategory::Error).unwrap() <= Duration::from_secs(120));
        assert!(rl.is_disabled(RateLimitingCategory::Session).unwrap() <= Duration::from_secs(60));
        assert!(rl.is_disabled(RateLimitingCategory::Transaction).is_none());
        assert!(rl.is_disabled(RateLimitingCategory::Any).is_none());

        rl.update_from_sentry_header(
            r#"
                30::bar, 
                120:invalid:invalid, 
                4711:foo;bar;baz;security:project
            "#,
        );

        assert!(
            rl.is_disabled(RateLimitingCategory::Transaction).unwrap() <= Duration::from_secs(30)
        );
        assert!(rl.is_disabled(RateLimitingCategory::Any).unwrap() <= Duration::from_secs(30));
    }

    #[test]
    fn test_retry_after() {
        let mut rl = RateLimiter::new();
        rl.update_from_retry_after("60");

        assert!(rl.is_disabled(RateLimitingCategory::Error).unwrap() <= Duration::from_secs(60));
        assert!(rl.is_disabled(RateLimitingCategory::Session).unwrap() <= Duration::from_secs(60));
        assert!(
            rl.is_disabled(RateLimitingCategory::Transaction).unwrap() <= Duration::from_secs(60)
        );
        assert!(rl.is_disabled(RateLimitingCategory::Any).unwrap() <= Duration::from_secs(60));
    }
}
