use sentry_core::protocol::Log;
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::log_from_event;

/// Captures `ERROR`, `WARN`, and `INFO` tracing events as Sentry logs.
///
/// Requires a client with logs enabled.
#[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
pub struct LogLayer<S> {
    with_span_attributes: bool,
    event_mapper: Option<Box<dyn EventToLogMapper<S>>>,
}

impl<S> LogLayer<S> {
    /// Include attributes of captured parent spans in logs.
    /// Requires a [`super::SpanLayer`] installed on the same subscriber.
    ///
    /// This option has no effect when [`Self::mapper`] is set.
    #[must_use]
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }

    /// Sets a custom mapping from the tracing event to the log object.
    pub fn mapper<M>(mut self, mapper: M) -> Self
    where
        M: EventToLogMapper<S> + 'static,
    {
        self.event_mapper = Some(Box::from(mapper));
        self
    }

    /// Capture a log on the current `Hub`.
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

impl<S> LogLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// Capture a tracing event as a log.
    pub(super) fn capture_tracing_event(&self, event: &Event, ctx: Context<'_, S>) {
        if let Some(log) = self.event_to_log(event, ctx) {
            self.capture(log);
        }
    }

    /// Convert the
    fn event_to_log(&self, event: &Event, ctx: Context<'_, S>) -> Option<Log> {
        match &self.event_mapper {
            Some(mapper) => mapper(event, ctx),
            None => Some(log_from_event(
                event,
                self.with_span_attributes.then_some(&ctx),
            )),
        }
    }
}

impl<S> Layer<S> for LogLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        self.capture_tracing_event(event, ctx);
    }
}

/// A mapper function which convernts a tracing event to a [`Log`].
///
/// This is a pretty advanced API; when using it, callers must handle constructing the log
/// object manually from the tracing event.
///
/// The function can also return [`None`], in which case, no log is created from the tracing event.
pub trait EventToLogMapper<S>: Fn(&Event, Context<'_, S>) -> Option<Log> + Send + Sync {}

impl<F, S> EventToLogMapper<S> for F where F: Fn(&Event, Context<'_, S>) -> Option<Log> + Send + Sync
{}

/// Creates a layer that captures Sentry logs from tracing events.
///
/// It is highly recommended to configure a level filter on the layer to limit log volume. We
/// recommend capturing logs at the `INFO` level, and above.
///
/// ```rust
/// # use tracing_subscriber::prelude::*;
/// use tracing_subscriber::filter::LevelFilter;
///
/// tracing_subscriber::registry()
///     .with(sentry::integrations::tracing::log_layer().with_filter(LevelFilter::INFO))
///     .init();
///
/// // This will be captured ...
/// tracing::info!("INFO log");
///
/// // ... but this will not be, due to LevelFilter::INFO being set.
/// tracing::debug!("DEBUG log");
/// ```
#[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
pub fn log_layer<S>() -> LogLayer<S> {
    LogLayer {
        with_span_attributes: false,
        event_mapper: None,
    }
}
