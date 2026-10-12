use sentry_core::Breadcrumb;
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::breadcrumb_from_event;

/// Adds tracing events as Sentry breadcrumbs.
///
/// Without a filter, events at every level become breadcrumbs. Configure a filter to limit
/// breadcrumb volume; see [`BreadcrumbLayer::new`] for the recommended levels.
///
/// Using this layer alongside the legacy [`SentryLayer`](super::SentryLayer) is unsupported.
/// Use either the legacy layer or the individual layers, not both.
#[must_use]
#[derive(Default)]
pub struct BreadcrumbLayer {
    with_span_attributes: bool,
}

impl BreadcrumbLayer {
    /// Creates a layer that adds tracing events as Sentry breadcrumbs.
    ///
    /// For most users, we would recommend using the [`LogLayer`](super::LogLayer) instead of this
    /// layer, as logs are sent regardless of whether an error occurs.
    ///
    /// Breadcrumbs are only sent attached to Sentry error events. Users using this layer may therefore
    /// also wish to enable the [`ErrorLayer`](super::ErrorLayer).
    ///
    /// # Filtering
    ///
    /// Without a filter, events at every level become breadcrumbs. Configure a filter to limit
    /// breadcrumb volume. We recommend capturing `WARN` and `INFO` events. If you also use the error layer, exclude `ERROR`
    /// events, which would otherwise duplicate the error event as a breadcrumb. If a custom filter
    /// sends the same event to both layers, install the breadcrumb layer before the error layer so
    /// that breadcrumb is included in the error.
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use sentry::integrations::tracing::{BreadcrumbLayer, ErrorLayer};
    /// use tracing::Level;
    /// use tracing_subscriber::filter::{filter_fn, LevelFilter};
    ///
    /// tracing_subscriber::registry()
    ///     .with(
    ///         BreadcrumbLayer::new().with_filter(filter_fn(|metadata| {
    ///             matches!(*metadata.level(), Level::WARN | Level::INFO)
    ///         })),
    ///     )
    ///     .with(ErrorLayer::new().with_filter(LevelFilter::ERROR))
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Include attributes of parent spans in breadcrumbs.
    ///
    /// We can only include spans captured by a [`super::SpanLayer`] on the same subscriber. So,
    /// this option only has any effect when the `SpanLayer` is installed.
    ///
    /// Furthermore, only spans accepted by the breadcrumb layer's filter (see
    /// [`tracing_subscriber::layer`]) are considered when attaching
    /// attributes. When using this option, therefore, we recommend building a custom filter that
    /// accepts all spans, while filtering events to the desired levels, like so:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// use sentry::integrations::tracing::{BreadcrumbLayer, SpanLayer};
    /// use tracing::Level;
    /// use tracing_subscriber::filter::{filter_fn, LevelFilter};
    ///
    /// tracing_subscriber::registry()
    ///     .with(SpanLayer::new().with_filter(LevelFilter::INFO))
    ///     .with(
    ///         BreadcrumbLayer::new()
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
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }

    /// Add a breadcrumb to the current `Hub`'s scope.
    pub(super) fn capture(&self, breadcrumb: Breadcrumb) {
        sentry_core::add_breadcrumb(breadcrumb);
    }
}

impl BreadcrumbLayer {
    /// Capture a tracing event as a breadcrumb.
    pub(super) fn capture_tracing_event<S>(&self, event: &Event, ctx: Context<'_, S>)
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        self.capture(breadcrumb_from_event(
            event,
            self.with_span_attributes.then_some(&ctx),
        ));
    }
}

impl<S> Layer<S> for BreadcrumbLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        self.capture_tracing_event(event, ctx);
    }
}
