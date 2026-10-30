use sentry_core::protocol::Event as SentryEvent;
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::event_from_event_impl;

/// Captures tracing events as Sentry error events.
///
/// Using this layer alongside the legacy [`SentryLayer`](super::SentryLayer) is unsupported.
/// Use either the legacy layer or the individual layers, not both.
#[must_use]
#[derive(Default)]
pub struct ErrorLayer {
    with_span_attributes: bool,
}

impl ErrorLayer {
    /// Creates a layer that captures tracing events as Sentry error events.
    ///
    /// We recommend enabling the [`SpanLayer`](super::SpanLayer) and [`LogLayer`](super::LogLayer).
    /// If you would like Sentry issues for error-level tracing events, you can also enable this
    /// layer. Error events count against your Sentry error quota, which is typically more expensive
    /// than logs.
    ///
    /// # Filtering
    ///
    /// Configure a level filter to limit error-event volume. We recommend capturing only `ERROR`
    /// events, as shown below.
    ///
    /// <div class="warning">
    ///
    /// **Do not use this layer without a filter.** An unfiltered layer captures every tracing
    /// event, including `DEBUG` and `TRACE`, as a Sentry error event. This can quickly exhaust
    /// your Sentry error quota.
    ///
    /// </div>
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use sentry::integrations::tracing::ErrorLayer;
    /// use tracing_subscriber::filter::LevelFilter;
    ///
    /// tracing_subscriber::registry()
    ///     .with(ErrorLayer::new().with_filter(LevelFilter::ERROR))
    ///     .init();
    ///
    /// // This will be captured ...
    /// tracing::error!("ERROR event");
    ///
    /// // ... but this will not be, due to LevelFilter::ERROR being set.
    /// tracing::warn!("WARN event");
    /// ```
    ///
    /// # Event conversion
    ///
    /// With the default converter:
    ///
    /// - String, numeric, and boolean fields prefixed with `tags.` become Sentry tags, with the
    ///   prefix removed.
    /// - Other data fields are stored in the `Rust Tracing Fields` context on the Sentry error event.
    /// - Record an error as `&dyn std::error::Error` to capture its exception type and source chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Include attributes of parent spans in error events.
    ///
    /// We can only include spans captured by a [`super::SpanLayer`] on the same subscriber. So,
    /// this option only has any effect when the `SpanLayer` is installed.
    ///
    /// # Filtering
    ///
    /// Parent-span attributes are included only when the span is accepted by both `SpanLayer`
    /// and `ErrorLayer`. A level filter such as `LevelFilter::ERROR` therefore omits attributes
    /// from less severe spans.
    ///
    /// To include attributes from all spans captured by `SpanLayer`, we recommend accepting all
    /// spans in this layer's filter, while filtering events to the desired level, like so:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use sentry::integrations::tracing::{ErrorLayer, SpanLayer};
    /// use tracing_subscriber::filter::{filter_fn, LevelFilter, FilterExt as _};
    ///
    /// tracing_subscriber::registry()
    ///     .with(SpanLayer::new().with_filter(LevelFilter::INFO))
    ///     .with(
    ///         ErrorLayer::new()
    ///             .enable_span_attributes()
    ///             .with_filter(LevelFilter::ERROR.or(filter_fn(|m| m.is_span()))),
    ///     )
    ///     .init();
    /// ```
    ///
    /// Including all spans in the error layer filter is safe, because this layer does not send
    /// any spans to Sentry.
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }

    /// Capture an error event on the current `Hub`.
    pub(super) fn capture(&self, event: SentryEvent<'static>) {
        sentry_core::capture_event(event);
    }

    /// Capture a tracing event as a Sentry error event.
    pub(super) fn capture_tracing_event<S>(&self, event: &Event, ctx: Context<'_, S>)
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let sentry_event = event_from_event_impl(event, self.with_span_attributes.then_some(&ctx));
        self.capture(sentry_event);
    }
}

impl<S> Layer<S> for ErrorLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        self.capture_tracing_event(event, ctx);
    }
}
