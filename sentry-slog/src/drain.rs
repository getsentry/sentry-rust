use crate::SlogIntegration;
use sentry_core::Hub;
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

// TODO: move this into `sentry-core`, as this is generally useful for more
// integrations.
fn with_integration<F, R>(f: F) -> R
where
    F: Fn(&Hub, &SlogIntegration) -> R,
    R: Default,
{
    Hub::with_active(|hub| hub.with_integration(|integration| f(hub, integration)))
}

impl<D: Drain> slog::Drain for SentryDrain<D> {
    type Ok = D::Ok;
    type Err = D::Err;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        with_integration(|hub, integration| integration.log(hub, record, values));
        self.drain.log(record, values)
    }

    fn is_enabled(&self, level: slog::Level) -> bool {
        with_integration(|_, integration| integration.is_enabled(level))
            || self.drain.is_enabled(level)
    }
}
