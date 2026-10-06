use sentry_core::protocol::Event as SentryEvent;
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::event_from_event;

/// Captures tracing events as Sentry error events.
///
/// # Warning
///
/// Without a filter, events at every level become Sentry error events. This can quickly exhaust
/// your Sentry error quota. Configure a filter to select the events to capture; see [`error_layer`].
pub struct ErrorLayer<S> {
    with_span_attributes: bool,
    event_mapper: Option<Box<dyn EventToErrorMapper<S>>>,
}

impl<S> ErrorLayer<S> {
    /// Include attributes of parent spans in error events.
    ///
    /// We can only include spans captured by a [`super::SpanLayer`] on the same subscriber. So,
    /// this option only has any effect when the `SpanLayer` is installed.
    ///
    /// Furthermore, only spans accepted by the error layer's filter (see
    /// [`tracing_subscriber::layer`]) are considered when attaching
    /// attributes. When using this option, therefore, we recommend building a custom filter that
    /// accepts all spans, while filtering errors to the desired level, like so:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use sentry::integrations::tracing as sentry_tracing;
    /// use tracing::Level;
    /// use tracing_subscriber::filter::{filter_fn, LevelFilter};
    ///
    /// tracing_subscriber::registry()
    ///     .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
    ///     .with(
    ///         sentry_tracing::error_layer()
    ///             .enable_span_attributes()
    ///             .with_filter(filter_fn(|metadata| {
    ///                 // Accept all spans, but only `ERROR` events.
    ///                 metadata.is_span() || *metadata.level() <= Level::ERROR
    ///             })),
    ///     )
    ///     .init();
    /// ```
    ///
    /// Including all spans in the error layer filter is safe, because this layer does not send
    /// any spans to Sentry.
    ///
    /// This option has no effect when [`Self::mapper`] is set.
    #[must_use]
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }

    /// Sets a custom mapping from the tracing event to the Sentry error event object.
    #[must_use]
    pub fn mapper<M>(mut self, mapper: M) -> Self
    where
        M: EventToErrorMapper<S> + 'static,
    {
        self.event_mapper = Some(Box::from(mapper));
        self
    }

    /// Capture an error event on the current `Hub`.
    pub(super) fn capture(&self, event: SentryEvent<'static>) {
        sentry_core::capture_event(event);
    }
}

impl<S> ErrorLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// Capture a tracing event as a Sentry error event.
    pub(super) fn capture_tracing_event(&self, event: &Event, ctx: Context<'_, S>) {
        if let Some(sentry_event) = self.event_to_error(event, ctx) {
            self.capture(sentry_event);
        }
    }

    /// Convert a tracing event to a Sentry error event, using the custom mapper if one is set.
    fn event_to_error(&self, event: &Event, ctx: Context<'_, S>) -> Option<SentryEvent<'static>> {
        match &self.event_mapper {
            Some(mapper) => mapper(event, ctx),
            None => Some(event_from_event(
                event,
                self.with_span_attributes.then_some(&ctx),
            )),
        }
    }
}

impl<S> Layer<S> for ErrorLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        self.capture_tracing_event(event, ctx);
    }
}

/// A mapper function which converts a tracing event to a Sentry error event.
///
/// Fully customizes if and how `tracing` events are converted to Sentry data.
///
/// The function can also return [`None`], in which case, no error event is created from the
/// tracing event.
pub trait EventToErrorMapper<S>:
    Fn(&Event, Context<'_, S>) -> Option<SentryEvent<'static>> + Send + Sync
{
}

impl<F, S> EventToErrorMapper<S> for F where
    F: Fn(&Event, Context<'_, S>) -> Option<SentryEvent<'static>> + Send + Sync
{
}

/// Creates a layer that captures tracing events as Sentry error events.
///
/// Most applications should start with [`log_layer`](super::log_layer), and add this layer if
/// `ERROR` events should also create Sentry issues. Combine it with
/// [`breadcrumb_layer`](super::breadcrumb_layer) to attach the events leading up to each error.
///
/// # Filtering
///
/// **Warning:** Without a filter, events at every level, including `DEBUG` and `TRACE`, become
/// Sentry error events. This can quickly exhaust your Sentry error quota. Configure a filter
/// so that only `ERROR` events create Sentry issues, as shown below.
///
/// ```rust
/// # use tracing_subscriber::prelude::*;
/// use sentry::integrations::tracing as sentry_tracing;
/// use tracing_subscriber::filter::LevelFilter;
///
/// tracing_subscriber::registry()
///     .with(sentry_tracing::error_layer().with_filter(LevelFilter::ERROR))
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
pub fn error_layer<S>() -> ErrorLayer<S> {
    ErrorLayer {
        with_span_attributes: false,
        event_mapper: None,
    }
}
