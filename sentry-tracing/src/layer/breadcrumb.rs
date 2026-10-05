use sentry_core::Breadcrumb;
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::breadcrumb_from_event;

/// Adds tracing events as Sentry breadcrumbs.
pub struct BreadcrumbLayer<S> {
    with_span_attributes: bool,
    event_mapper: Option<Box<dyn EventToBreadcrumbMapper<S>>>,
}

impl<S> BreadcrumbLayer<S> {
    /// Include attributes of parent spans in breadcrumbs.
    ///
    /// We can only include spans captured by a [`super::SpanLayer`] on the same subscriber. So,
    /// this option only has any effect when the `SpanLayer` is installed.
    ///
    /// Furthermore, only spans accepted by the breadcrumb layer's
    /// [filter](tracing_subscriber::layer#per-layer-filtering) are considered when attaching
    /// attributes. When using this option, therefore, we recommend building a custom filter that
    /// accepts all spans, while filtering events to the desired levels, like so:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use tracing::Level;
    /// use tracing_subscriber::filter::{filter_fn, LevelFilter};
    ///
    /// tracing_subscriber::registry()
    ///     .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
    ///     .with(
    ///         sentry_tracing::breadcrumb_layer()
    ///             .enable_span_attributes()
    ///             .with_filter(filter_fn(|metadata| {
    ///                 // Accept all spans, but only `WARN` and `INFO` events.
    ///                 metadata.is_span() || matches!(*metadata.level(), Level::WARN | Level::INFO)
    ///             })),
    ///     )
    ///     .init();
    /// ```
    ///
    /// Including all spans in the breadcrumb layer filter is safe, because this layer does not send
    /// any spans to Sentry.
    ///
    /// This option has no effect when [`Self::mapper`] is set.
    #[must_use]
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }

    /// Sets a custom mapping from the tracing event to the breadcrumb object.
    #[must_use]
    pub fn mapper<M>(mut self, mapper: M) -> Self
    where
        M: EventToBreadcrumbMapper<S> + 'static,
    {
        self.event_mapper = Some(Box::from(mapper));
        self
    }

    /// Add a breadcrumb to the current `Hub`'s scope.
    pub(super) fn capture(&self, breadcrumb: Breadcrumb) {
        sentry_core::add_breadcrumb(breadcrumb);
    }
}

impl<S> BreadcrumbLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// Capture a tracing event as a breadcrumb.
    pub(super) fn capture_tracing_event(&self, event: &Event, ctx: Context<'_, S>) {
        if let Some(breadcrumb) = self.event_to_breadcrumb(event, ctx) {
            self.capture(breadcrumb);
        }
    }

    /// Convert a tracing event to a breadcrumb, using the custom mapper if one is set.
    fn event_to_breadcrumb(&self, event: &Event, ctx: Context<'_, S>) -> Option<Breadcrumb> {
        match &self.event_mapper {
            Some(mapper) => mapper(event, ctx),
            None => Some(breadcrumb_from_event(
                event,
                self.with_span_attributes.then_some(&ctx),
            )),
        }
    }
}

impl<S> Layer<S> for BreadcrumbLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        self.capture_tracing_event(event, ctx);
    }
}

/// A mapper function which converts a tracing event to a [`Breadcrumb`].
///
/// This is a pretty advanced API; when using it, callers must handle constructing the breadcrumb
/// object manually from the tracing event.
///
/// The function can also return [`None`], in which case, no breadcrumb is created from the
/// tracing event.
pub trait EventToBreadcrumbMapper<S>:
    Fn(&Event, Context<'_, S>) -> Option<Breadcrumb> + Send + Sync
{
}

impl<F, S> EventToBreadcrumbMapper<S> for F where
    F: Fn(&Event, Context<'_, S>) -> Option<Breadcrumb> + Send + Sync
{
}

/// Creates a layer that adds tracing events as Sentry breadcrumbs.
///
/// For most users, we would recommend using the [`log_layer`](super::log_layer) instead of this
/// layer, as logs are trace-connected and sent regardless of whether an error occurs.
///
/// Breadcrumbs are only sent attached to Sentry error events. Users using this layer may therefore
/// also wish to enable the [`error_layer`](super::error_layer).
///
/// # Filtering
///
/// It is highly recommended to configure a filter on the layer to limit breadcrumb volume. We
/// recommend capturing `WARN` and `INFO` events. If you also use the error layer, exclude `ERROR`
/// events, which would otherwise duplicate the error event as a breadcrumb. If a custom filter
/// sends the same event to both layers, install the breadcrumb layer before the error layer so
/// that breadcrumb is included in the error.
///
/// ```rust
/// # use tracing_subscriber::prelude::*;
/// use tracing::Level;
/// use tracing_subscriber::filter::{filter_fn, LevelFilter};
///
/// tracing_subscriber::registry()
///     .with(
///         sentry_tracing::breadcrumb_layer().with_filter(filter_fn(|metadata| {
///             matches!(*metadata.level(), Level::WARN | Level::INFO)
///         })),
///     )
///     .with(sentry_tracing::error_layer().with_filter(LevelFilter::ERROR))
///     .init();
///
/// // These will be captured as breadcrumbs ...
/// tracing::info!("INFO breadcrumb");
/// tracing::warn!("WARN breadcrumb");
///
/// // ... but this will not be.
/// tracing::debug!("DEBUG breadcrumb");
///
/// // This will not be captured as a breadcrumb, but as an error event by the error layer. The
/// // breadcrumbs above will be contained in that error event.
/// tracing::error!("ERROR event");
/// ```
pub fn breadcrumb_layer<S>() -> BreadcrumbLayer<S> {
    BreadcrumbLayer {
        with_span_attributes: false,
        event_mapper: None,
    }
}
