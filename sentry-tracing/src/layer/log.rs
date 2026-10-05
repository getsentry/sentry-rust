use sentry_core::protocol::Log;
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::log_from_event;

/// Captures tracing events as Sentry logs.
///
/// Captures all event levels unless a filter is configured; see [`log_layer`] for the recommended
/// level filter.
///
/// Requires a client with logs enabled.
#[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
pub struct LogLayer<S> {
    with_span_attributes: bool,
    event_mapper: Option<Box<dyn EventToLogMapper<S>>>,
}

impl<S> LogLayer<S> {
    /// Include attributes of parent spans in logs.
    ///
    /// We can only include spans captured by a [`super::SpanLayer`] on the same subscriber. So,
    /// this option only has any effect when the `SpanLayer` is installed.
    ///
    /// Furthermore, only spans accepted by the log layer's
    /// [filter](tracing_subscriber::layer#per-layer-filtering) are considered when attaching
    /// attributes. When using this option, therefore, we recommend building a custom filter that
    /// accepts all spans, while filtering events to the desired level, like so:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use tracing::Level;
    /// use tracing_subscriber::filter::{filter_fn, LevelFilter};
    ///
    /// tracing_subscriber::registry()
    ///     .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
    ///     .with(
    ///         sentry_tracing::log_layer()
    ///             .enable_span_attributes()
    ///             .with_filter(filter_fn(|metadata| {
    ///                 // Accept all spans, but only events at `INFO` and more severe.
    ///                 metadata.is_span() || *metadata.level() <= Level::INFO
    ///             })),
    ///     )
    ///     .init();
    /// ```
    ///
    /// Including all spans in the log layer filter is safe, because this layer does not send any
    /// spans to Sentry.
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

/// A mapper function which converts a tracing event to a [`Log`].
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
/// # Filtering
///
/// It is highly recommended to configure a level filter on the layer to limit log volume. We
/// recommend capturing logs at the `INFO` level, and more severe.
///
/// ```rust
/// # use tracing_subscriber::prelude::*;
/// use tracing_subscriber::filter::LevelFilter;
///
/// tracing_subscriber::registry()
///     .with(sentry_tracing::log_layer().with_filter(LevelFilter::INFO))
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
