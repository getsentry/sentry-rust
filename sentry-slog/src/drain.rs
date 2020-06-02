use crate::SlogIntegration;
use slog::{Drain, OwnedKVList, Record};

/// A Drain which passes all Records to sentry.
pub struct SentryDrain<D: Drain> {
    drain: D,
}

impl<D: Drain> SentryDrain<D> {
    /// Creates a new `SentryDrain`, wrapping a `slog::Drain`.
    pub fn new(drain: D) -> Self {
        Self { drain }
    }
}

impl<D: Drain> slog::Drain for SentryDrain<D> {
    type Ok = D::Ok;
    type Err = D::Err;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        sentry_core::with_integration(|integration: &SlogIntegration, hub| {
            integration.log(hub, record, values)
        });
        self.drain.log(record, values)
    }

    fn is_enabled(&self, level: slog::Level) -> bool {
        sentry_core::with_integration(|integration: &SlogIntegration, _| {
            integration.is_enabled(level)
        }) || self.drain.is_enabled(level)
    }
}
