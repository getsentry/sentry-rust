use bitflags::bitflags;
use sentry_core::Breadcrumb;
use tracing_core::{span as tracing_span, Event, Level, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

mod breadcrumb;
mod error;
#[cfg(feature = "logs")]
mod log;
mod span;
mod span_guard_stack;

pub use breadcrumb::{breadcrumb_layer, BreadcrumbLayer};
pub use error::{error_layer, ErrorLayer};
#[cfg(feature = "logs")]
pub use log::{log_layer, LogLayer};
pub(super) use span::SentrySpanData;
pub use span::{span_layer, SpanLayer};

// TODO: Design new-layer filtering without hiding filtered spans from event
// context lookups. A per-layer tracing-subscriber Filter only narrows what
// that layer receives; it is not a replacement for legacy custom mappings.

bitflags! {
    /// The action that Sentry should perform for a given [`Event`]
    #[derive(Debug, Clone, Copy)]
    pub struct EventFilter: u32 {
        /// Ignore the [`Event`]
        const Ignore = 0b000;
        /// Create a [`Breadcrumb`] from this [`Event`]
        const Breadcrumb = 0b001;
        /// Create a [`sentry_core::protocol::Event`] from this [`Event`]
        const Event = 0b010;
        /// Create a [`sentry_core::protocol::Log`] from this [`Event`]
        const Log = 0b100;
    }
}

/// The type of data Sentry should ingest for an [`Event`].
#[derive(Debug)]
#[non_exhaustive]
pub enum EventMapping {
    /// Ignore the [`Event`]
    Ignore,
    /// Adds the [`Breadcrumb`] to the Sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the [`sentry_core::protocol::Event`] to Sentry.
    Event(Box<sentry_core::protocol::Event<'static>>),
    /// Captures the [`sentry_core::protocol::Log`] to Sentry.
    #[cfg(feature = "logs")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "logs")))]
    Log(sentry_core::protocol::Log),
    /// Captures multiple items to Sentry.
    /// Nesting multiple `EventMapping::Combined` inside each other will cause the inner mappings to be ignored.
    Combined(CombinedEventMapping),
}

/// A list of event mappings.
#[derive(Debug)]
pub struct CombinedEventMapping(Vec<EventMapping>);

impl From<EventMapping> for CombinedEventMapping {
    fn from(value: EventMapping) -> Self {
        match value {
            EventMapping::Combined(combined) => combined,
            _ => CombinedEventMapping(vec![value]),
        }
    }
}

impl From<Vec<EventMapping>> for CombinedEventMapping {
    fn from(value: Vec<EventMapping>) -> Self {
        Self(value)
    }
}

/// The default event filter for the legacy combined layer.
///
/// By default, an exception event is captured for `error`, a breadcrumb for
/// `warning` and `info`, and `debug` and `trace` logs are ignored.
pub fn default_event_filter(metadata: &Metadata) -> EventFilter {
    match metadata.level() {
        #[cfg(feature = "logs")]
        &Level::ERROR => EventFilter::Event | EventFilter::Log,
        #[cfg(not(feature = "logs"))]
        &Level::ERROR => EventFilter::Event,
        #[cfg(feature = "logs")]
        &Level::WARN | &Level::INFO => EventFilter::Breadcrumb | EventFilter::Log,
        #[cfg(not(feature = "logs"))]
        &Level::WARN | &Level::INFO => EventFilter::Breadcrumb,
        &Level::DEBUG | &Level::TRACE => EventFilter::Ignore,
    }
}

/// The default span filter.
///
/// By default, spans at the `error`, `warning`, and `info` levels are captured.
pub fn default_span_filter(metadata: &Metadata) -> bool {
    matches!(
        metadata.level(),
        &Level::ERROR | &Level::WARN | &Level::INFO
    )
}

type EventMapper<S> = Box<dyn Fn(&Event, Context<'_, S>) -> EventMapping + Send + Sync>;

/// Legacy combined tracing layer; use [`span_layer`], [`breadcrumb_layer`],
/// [`error_layer`], and `log_layer()` instead.
#[deprecated(
    note = "Use span_layer(), breadcrumb_layer(), error_layer(), and optionally log_layer() instead"
)]
pub struct SentryLayer<S> {
    event_filter: Box<dyn Fn(&Metadata) -> EventFilter + Send + Sync>,
    event_mapper: Option<EventMapper<S>>,
    span_layer: SpanLayer,
    breadcrumb: BreadcrumbLayer,
    error: ErrorLayer,
    #[cfg(feature = "logs")]
    log: LogLayer,
}

#[expect(deprecated, reason = "implementing the legacy layer")]
impl<S> SentryLayer<S> {
    /// Sets a custom event filter function.
    ///
    /// The filter classifies how sentry should handle [`Event`]s based
    /// on their [`Metadata`].
    #[must_use]
    pub fn event_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Metadata) -> EventFilter + Send + Sync + 'static,
    {
        self.event_filter = Box::new(filter);
        self
    }

    /// Sets a custom event mapper function.
    ///
    /// The mapper is responsible for creating either breadcrumbs or events from
    /// [`Event`]s.
    #[must_use]
    pub fn event_mapper<F>(mut self, mapper: F) -> Self
    where
        F: Fn(&Event, Context<'_, S>) -> EventMapping + Send + Sync + 'static,
    {
        self.event_mapper = Some(Box::new(mapper));
        self
    }

    /// Sets a custom span filter function.
    ///
    /// The filter classifies whether sentry should handle [`tracing::Span`]s based
    /// on their [`Metadata`].
    ///
    /// [`tracing::Span`]: https://docs.rs/tracing/latest/tracing/struct.Span.html
    #[must_use]
    pub fn span_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Metadata) -> bool + Send + Sync + 'static,
    {
        self.span_layer.span_filter(filter);
        self
    }

    /// Enable every parent span's attributes to be sent along with own event's attributes.
    ///
    /// Note that the root span is considered a [transaction][sentry_core::protocol::Transaction]
    /// so its context will only be grabbed only if you set the transaction to be sampled.
    /// The most straightforward way to do this is to set
    /// the [traces_sample_rate][sentry_core::ClientOptions::traces_sample_rate] to `1.0`
    /// while configuring your sentry client.
    #[must_use]
    pub fn enable_span_attributes(mut self) -> Self {
        self.breadcrumb = self.breadcrumb.enable_span_attributes();
        self.error = self.error.enable_span_attributes();
        #[cfg(feature = "logs")]
        {
            self.log = self.log.enable_span_attributes();
        }
        self
    }
}

#[expect(deprecated, reason = "implementing the legacy layer")]
impl<S> Default for SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn default() -> Self {
        Self {
            event_filter: Box::new(default_event_filter),
            event_mapper: None,
            span_layer: span_layer(),
            breadcrumb: breadcrumb_layer(),
            error: error_layer(),
            #[cfg(feature = "logs")]
            log: log_layer(),
        }
    }
}

#[expect(deprecated, reason = "implementing the legacy layer")]
impl<S> Layer<S> for SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        if let Some(mapper) = &self.event_mapper {
            // A custom mapper overrides the filter and supplies already converted
            // items in the order they must be captured.
            let items = CombinedEventMapping::from(mapper(event, ctx));
            for item in items.0 {
                match item {
                    EventMapping::Ignore => (),
                    EventMapping::Breadcrumb(breadcrumb) => self.breadcrumb.capture(breadcrumb),
                    EventMapping::Event(event) => self.error.capture(*event),
                    #[cfg(feature = "logs")]
                    EventMapping::Log(log) => self.log.capture(log),
                    EventMapping::Combined(_) => sentry_core::sentry_debug!(
                        "[SentryLayer] found nested CombinedEventMapping, ignoring"
                    ),
                }
            }
        } else {
            // The legacy filter may choose nondefault levels. Do not reapply the
            // standalone event layers' default level checks here.
            let filter = (self.event_filter)(event.metadata());
            if filter.contains(EventFilter::Breadcrumb) {
                self.breadcrumb.capture_event(event, &ctx);
            }
            if filter.contains(EventFilter::Event) {
                self.error.capture_event(event, &ctx);
            }
            #[cfg(feature = "logs")]
            if filter.contains(EventFilter::Log) {
                self.log.capture_event(event, &ctx);
            }
        }
    }

    fn on_new_span(
        &self,
        attrs: &tracing_span::Attributes<'_>,
        id: &tracing_span::Id,
        ctx: Context<'_, S>,
    ) {
        self.span_layer.on_new_span(attrs, id, ctx);
    }

    fn on_enter(&self, id: &tracing_span::Id, ctx: Context<'_, S>) {
        self.span_layer.on_enter(id, ctx);
    }

    fn on_exit(&self, id: &tracing_span::Id, ctx: Context<'_, S>) {
        self.span_layer.on_exit(id, ctx);
    }

    fn on_close(&self, id: tracing_span::Id, ctx: Context<'_, S>) {
        self.span_layer.on_close(id, ctx);
    }

    fn on_record(
        &self,
        id: &tracing_span::Id,
        values: &tracing_span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        self.span_layer.on_record(id, values, ctx);
    }
}

/// Creates a legacy combined Sentry layer. Use individual telemetry layers instead.
#[deprecated(
    note = "Use span_layer(), breadcrumb_layer(), error_layer(), and optionally log_layer() instead"
)]
#[expect(deprecated, reason = "returning the legacy layer")]
pub fn layer<S>() -> SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    Default::default()
}
