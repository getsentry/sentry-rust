use sentry_core::protocol::Breadcrumb;
use tracing_core::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

use crate::converters::*;

/// The action that Sentry should perform for a [`Metadata`]
#[derive(Debug, Clone, Copy)]
pub enum EventFilter {
    /// Ignore the [`Event`]
    Ignore,
    /// Create a [`Breadcrumb`] from this [`Event`]
    Breadcrumb,
    /// Create a message [`sentry_core::protocol::Event`] from this [`Event`]
    Event,
    /// Create an exception [`sentry_core::protocol::Event`] from this [`Event`]
    Exception,
}

/// The type of data Sentry should ingest for a [`Event`]
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EventMapping {
    /// Ignore the [`Event`]
    Ignore,
    /// Adds the [`Breadcrumb`] to the Sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the [`sentry_core::protocol::Event`] to Sentry.
    Event(sentry_core::protocol::Event<'static>),
}

/// The default event filter.
///
/// By default, an exception event is captured for `error`, a breadcrumb for
/// `warning` and `info`, and `debug` and `trace` logs are ignored.
pub fn default_filter(metadata: &Metadata) -> EventFilter {
    match metadata.level() {
        &Level::ERROR => EventFilter::Exception,
        &Level::WARN | &Level::INFO => EventFilter::Breadcrumb,
        &Level::DEBUG | &Level::TRACE => EventFilter::Ignore,
    }
}

/// Provides a tracing layer that dispatches events to sentry
pub struct SentryLayer {
    filter: Box<dyn Fn(&Metadata) -> EventFilter + Send + Sync>,
    mapper: Option<Box<dyn Fn(&Event) -> EventMapping + Send + Sync>>,
}

impl SentryLayer {
    /// Sets a custom filter function.
    ///
    /// The filter classifies how sentry should handle [`Event`]s based
    /// on their [`Metadata`].
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Metadata) -> EventFilter + Send + Sync + 'static,
    {
        self.filter = Box::new(filter);
        self
    }

    /// Sets a custom mapper function.
    ///
    /// The mapper is responsible for creating either breadcrumbs or events from
    /// [`Event`]s.
    pub fn mapper<F>(mut self, mapper: F) -> Self
    where
        F: Fn(&Event) -> EventMapping + Send + Sync + 'static,
    {
        self.mapper = Some(Box::new(mapper));
        self
    }
}

impl Default for SentryLayer {
    fn default() -> Self {
        Self {
            filter: Box::new(default_filter),
            mapper: None,
        }
    }
}

impl<S: Subscriber> Layer<S> for SentryLayer {
    fn on_event(&self, event: &Event, _ctx: Context<'_, S>) {
        let item = match &self.mapper {
            Some(mapper) => mapper(event),
            None => match (self.filter)(event.metadata()) {
                EventFilter::Ignore => EventMapping::Ignore,
                EventFilter::Breadcrumb => EventMapping::Breadcrumb(breadcrumb_from_event(event)),
                EventFilter::Event => EventMapping::Event(event_from_event(event)),
                EventFilter::Exception => EventMapping::Event(exception_from_event(event)),
            },
        };

        match item {
            EventMapping::Event(event) => {
                sentry_core::capture_event(event);
            }
            EventMapping::Breadcrumb(breadcrumb) => sentry_core::add_breadcrumb(breadcrumb),
            _ => (),
        }
    }
}

/// Creates a default Sentry layer
pub fn layer() -> SentryLayer {
    Default::default()
}
