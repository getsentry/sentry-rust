use sentry_core::protocol::Log;
use tracing_core::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::log_from_event;

/// Captures `ERROR`, `WARN`, and `INFO` tracing events as Sentry logs.
///
/// Requires a client with logs enabled.
#[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
#[derive(Default)]
pub struct LogLayer {
    with_span_attributes: bool,
}

impl LogLayer {
    /// Include attributes of captured parent spans in logs.
    /// Requires a [`super::SpanLayer`] installed on the same subscriber.
    #[must_use]
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }

    pub(super) fn capture_event<S>(&self, event: &Event, ctx: &Context<'_, S>)
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        self.capture(log_from_event(
            event,
            self.with_span_attributes.then_some(ctx),
        ));
    }

    pub(super) fn capture(&self, log: Log) {
        sentry_core::Hub::with_active(|hub| {
            let enabled = hub.client().is_none_or(|client| {
                let options = client.options();
                #[expect(deprecated, reason = "checking a deprecated field")]
                options.enable_logs
            });
            if enabled {
                hub.capture_log(log);
            }
        });
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        if matches!(
            event.metadata().level(),
            &Level::ERROR | &Level::WARN | &Level::INFO
        ) {
            self.capture_event(event, &ctx);
        }
    }
}

/// Creates a layer that captures only Sentry logs.
#[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
pub fn log_layer() -> LogLayer {
    LogLayer::default()
}
