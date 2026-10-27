use sentry_core::protocol::Log;
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::log_from_event_impl;

/// Captures tracing events as Sentry logs.
///
/// Captures all event levels unless a filter is configured; see [`LogLayer::new`] for the recommended
/// level filter.
///
/// Logs are sent by default, unless disabled.
///
/// Using this layer alongside the legacy [`SentryLayer`](super::SentryLayer) is unsupported.
/// Use either the legacy layer or the individual layers, not both.
#[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
#[must_use]
#[derive(Default)]
pub struct LogLayer {
    with_span_attributes: bool,
}

impl LogLayer {
    /// Creates a layer that captures Sentry logs from tracing events.
    ///
    /// # Filtering
    ///
    /// Without a filter, this layer captures logs at every level. Configure a level filter to limit
    /// log volume. We recommend capturing logs at the `INFO` level, and more severe.
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use sentry::integrations::tracing::LogLayer;
    /// use tracing_subscriber::filter::LevelFilter;
    ///
    /// tracing_subscriber::registry()
    ///     .with(LogLayer::new().with_filter(LevelFilter::INFO))
    ///     .init();
    ///
    /// // This will be captured ...
    /// tracing::info!("INFO log");
    ///
    /// // ... but this will not be, due to LevelFilter::INFO being set.
    /// tracing::debug!("DEBUG log");
    /// ```
    #[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
    pub fn new() -> Self {
        Self::default()
    }

    /// Include attributes of parent spans in logs.
    ///
    /// We can only include spans captured by a [`super::SpanLayer`] on the same subscriber. So,
    /// this option only has any effect when the `SpanLayer` is installed.
    ///
    /// # Filtering
    ///
    /// Parent-span attributes are included only when the span is accepted by both `SpanLayer`
    /// and `LogLayer`.
    ///
    /// If `LogLayer` uses a stricter level filter, attributes from less severe spans are omitted.
    /// For example, this captures logs at `WARN` and more severe levels while allowing attributes
    /// from `INFO` parent spans to appear on those logs:
    ///
    /// ```rust
    /// use sentry::integrations::tracing::{LogLayer, SpanLayer};
    /// use tracing_subscriber::prelude::*;
    /// use tracing_subscriber::filter::{filter_fn, FilterExt as _, LevelFilter};
    ///
    /// tracing_subscriber::registry()
    ///     .with(SpanLayer::new().with_filter(LevelFilter::INFO))
    ///     .with(
    ///         LogLayer::new()
    ///             .enable_span_attributes()
    ///             .with_filter(LevelFilter::WARN.or(filter_fn(|m| m.is_span()))),
    ///     )
    ///     .init();
    /// ```
    ///
    /// No filter adjustment is needed when both layers use the same level filter, such as the
    /// `INFO` level recommended for most applications.
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
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

impl LogLayer {
    /// Capture a tracing event as a log.
    pub(super) fn capture_tracing_event<S>(&self, event: &Event, ctx: Context<'_, S>)
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        self.capture(log_from_event_impl(
            event,
            self.with_span_attributes.then_some(&ctx),
        ));
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        self.capture_tracing_event(event, ctx);
    }
}
