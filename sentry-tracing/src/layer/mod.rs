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

pub use breadcrumb::{breadcrumb_layer, BreadcrumbLayer, EventToBreadcrumbMapper};
pub use error::{error_layer, ErrorLayer, EventToErrorMapper};
#[cfg(feature = "logs")]
pub use log::{log_layer, EventToLogMapper, LogLayer};
pub(super) use span::SentrySpanData;
pub use span::{span_layer, SpanLayer};

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
/// Captures `ERROR` events as Sentry error events and `WARN` and `INFO` events as breadcrumbs.
/// With the `logs` feature, these three levels are also captured as logs. `DEBUG` and `TRACE`
/// events are ignored.
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

/// Legacy combined tracing layer.
///
/// For most applications, use [`span_layer`] and [`log_layer`] instead.
/// Add [`error_layer`] if tracing events should also create Sentry issues, and [`breadcrumb_layer`]
/// if errors need breadcrumb context.
///
/// # Migrating the default configuration
///
/// The individual layers do not filter by level internally. The following configuration
/// reproduces the default level selection of `SentryLayer` with the `logs` feature enabled:
///
/// - `ERROR`, `WARN`, and `INFO` spans become Sentry transactions and spans.
/// - `ERROR`, `WARN`, and `INFO` events become structured logs.
/// - `ERROR` events also become Sentry error events.
/// - `WARN` and `INFO` events also become breadcrumbs on the current scope, sent with subsequent
///   captured errors.
///
/// ```rust
/// # #[cfg(feature = "logs")]
/// # {
/// use tracing::Level;
/// use tracing_subscriber::filter::{filter_fn, LevelFilter};
/// use tracing_subscriber::prelude::*;
///
/// tracing_subscriber::registry()
///     .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
///     .with(
///         sentry_tracing::breadcrumb_layer().with_filter(filter_fn(|metadata| {
///             matches!(*metadata.level(), Level::WARN | Level::INFO)
///         })),
///     )
///     .with(sentry_tracing::error_layer().with_filter(LevelFilter::ERROR))
///     .with(sentry_tracing::log_layer().with_filter(LevelFilter::INFO))
///     .init();
/// # }
/// ```
///
/// Without the `logs` feature, omit the log layer. Client configuration, including trace sampling,
/// is unchanged.
///
/// If the legacy layer used [`enable_span_attributes`](Self::enable_span_attributes), enable
/// span attributes on each individual event layer that needs them. See the option docs for
/// [breadcrumbs](BreadcrumbLayer::enable_span_attributes),
/// [errors](ErrorLayer::enable_span_attributes), and
/// [logs](LogLayer::enable_span_attributes) for filtering requirements.
///
/// # Migrating custom configuration
///
/// - Replace [`span_filter`](Self::span_filter) with a filter on the span layer.
/// - Split [`event_filter`](Self::event_filter) into filters on the individual event layers. An
///   event can be accepted by multiple layers, replacing combined [`EventFilter`] flags.
/// - Replace [`event_mapper`](Self::event_mapper) with each event layer's `mapper()` callback.
///   Unlike the legacy mapper, these callbacks do not override filters: filters run first and
///   can prevent a mapper from receiving an event. Keep value-dependent routing in the mapper.
/// - Each mapper returns at most one item of its telemetry type. There is no direct equivalent
///   for a legacy [`EventMapping::Combined`] that emits multiple items of the same type or changes
///   their capture order per event.
///
/// If a custom configuration sends the same event to both the breadcrumb and error layers,
/// install the breadcrumb layer before the error layer to include that breadcrumb in the error.
/// The default configuration above excludes `ERROR` events from breadcrumbs, so the order does
/// not affect their inclusion.
#[deprecated(note = "Prefer span_layer() and log_layer(); see SentryLayer docs for migration")]
pub struct SentryLayer<S> {
    event_filter: Box<dyn Fn(&Metadata) -> EventFilter + Send + Sync>,
    event_mapper: Option<EventMapper<S>>,
    span_filter: Box<dyn Fn(&Metadata) -> bool + Send + Sync>,
    span_layer: SpanLayer,
    breadcrumb: BreadcrumbLayer<S>,
    error: ErrorLayer<S>,
    #[cfg(feature = "logs")]
    log: LogLayer<S>,
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
        self.span_filter = Box::new(filter);
        self
    }

    /// Include attributes of parent spans in breadcrumbs, error events, and logs.
    ///
    /// Only spans accepted by this layer's span filter provide attributes. Including attributes
    /// does not require the transaction to be sampled.
    ///
    /// When migrating, enable span attributes on each individual event layer that needs them;
    /// see the [migration guide](SentryLayer#migrating-the-default-configuration).
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
            span_filter: Box::new(default_span_filter),
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
                self.breadcrumb.capture_tracing_event(event, ctx.clone());
            }
            if filter.contains(EventFilter::Event) {
                self.error.capture_tracing_event(event, ctx.clone());
            }
            #[cfg(feature = "logs")]
            if filter.contains(EventFilter::Log) {
                self.log.capture_tracing_event(event, ctx);
            }
        }
    }

    fn on_new_span(
        &self,
        attrs: &tracing_span::Attributes<'_>,
        id: &tracing_span::Id,
        ctx: Context<'_, S>,
    ) {
        if (self.span_filter)(attrs.metadata()) {
            self.span_layer.on_new_span(attrs, id, ctx);
        }
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

/// Creates a legacy combined Sentry layer.
///
/// For most applications, use [`span_layer`] and [`log_layer`] instead.
/// See [`SentryLayer`] for a migration example that preserves the legacy default level selection
/// using all four individual layers, and for differences when migrating custom configuration.
#[deprecated(note = "Prefer span_layer() and log_layer(); see SentryLayer docs for migration")]
#[expect(deprecated, reason = "returning the legacy layer")]
pub fn layer<S>() -> SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    Default::default()
}
