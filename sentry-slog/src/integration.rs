use sentry_core::Integration;

/// The Sentry `slog` Integration.
#[derive(Debug, Default)]
pub struct SlogIntegration {}

impl SlogIntegration {
    /// Create a new `slog` Integration.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Integration for SlogIntegration {
    fn name(&self) -> &'static str {
        "slog"
    }
}
