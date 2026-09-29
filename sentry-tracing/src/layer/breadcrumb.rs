use sentry_core::Breadcrumb;
use tracing_core::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::breadcrumb_from_event;

/// Adds `WARN` and `INFO` tracing events as Sentry breadcrumbs.
#[derive(Default)]
pub struct BreadcrumbLayer {
    with_span_attributes: bool,
}

impl BreadcrumbLayer {
    /// Include attributes of captured parent spans in breadcrumbs.
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
        self.capture(breadcrumb_from_event(
            event,
            self.with_span_attributes.then_some(ctx),
        ));
    }

    pub(super) fn capture(&self, breadcrumb: Breadcrumb) {
        sentry_core::add_breadcrumb(breadcrumb);
    }
}

impl<S> Layer<S> for BreadcrumbLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        if matches!(event.metadata().level(), &Level::WARN | &Level::INFO) {
            self.capture_event(event, &ctx);
        }
    }
}

/// Creates a layer that captures only breadcrumbs.
pub fn breadcrumb_layer() -> BreadcrumbLayer {
    BreadcrumbLayer::default()
}
