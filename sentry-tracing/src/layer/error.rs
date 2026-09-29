use sentry_core::protocol::Event as SentryEvent;
use tracing_core::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::event_from_event;

/// Captures `ERROR` tracing events as Sentry error events.
#[derive(Default)]
pub struct ErrorLayer {
    with_span_attributes: bool,
}

impl ErrorLayer {
    /// Include attributes of captured parent spans in error events.
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
        self.capture(event_from_event(
            event,
            self.with_span_attributes.then_some(ctx),
        ));
    }

    pub(super) fn capture(&self, event: SentryEvent<'static>) {
        sentry_core::capture_event(event);
    }
}

impl<S> Layer<S> for ErrorLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        if event.metadata().level() == &Level::ERROR {
            self.capture_event(event, &ctx);
        }
    }
}

/// Creates a layer that captures only Sentry error events.
pub fn error_layer() -> ErrorLayer {
    ErrorLayer::default()
}
