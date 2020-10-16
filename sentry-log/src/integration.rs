use sentry_core::{ClientOptions, Integration};

/// The Sentry [`log`] Integration.
#[derive(Default)]
pub struct LogIntegration;

impl Integration for LogIntegration {
    fn name(&self) -> &'static str {
        "log"
    }

    fn setup(&self, cfg: &mut ClientOptions) {
        cfg.in_app_exclude.push("log::");
        cfg.extra_border_frames
            .push("<sentry_log::Logger as log::Log>::log");
    }
}

impl LogIntegration {
    /// Creates a new `log` Integration.
    pub fn new() -> Self {
        Self::default()
    }
}
