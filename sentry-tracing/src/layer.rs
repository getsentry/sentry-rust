use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bitflags::bitflags;
use sentry_core::protocol::Value;
use sentry_core::{Breadcrumb, TransactionOrSpan};
use tracing_core::field::Visit;
use tracing_core::{span, Event, Field, Level, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::*;
use crate::SENTRY_NAME_FIELD;
use crate::SENTRY_OP_FIELD;
use crate::SENTRY_TRACE_FIELD;
use crate::TAGS_PREFIX;

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
#[allow(clippy::large_enum_variant)]
pub enum EventMapping {
    /// Ignore the [`Event`]
    Ignore,
    /// Adds the [`Breadcrumb`] to the Sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the [`sentry_core::protocol::Event`] to Sentry.
    Event(sentry_core::protocol::Event<'static>),
    /// Captures the [`sentry_core::protocol::Log`] to Sentry.
    #[cfg(feature = "logs")]
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

/// The default event filter.
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
/// By default, spans at the `error`, `warning`, and `info`
/// levels are captured
pub fn default_span_filter(metadata: &Metadata) -> bool {
    matches!(
        metadata.level(),
        &Level::ERROR | &Level::WARN | &Level::INFO
    )
}

type EventMapper<S> = Box<dyn Fn(&Event, Context<'_, S>) -> EventMapping + Send + Sync>;

/// Provides a tracing layer that dispatches events to sentry
pub struct SentryLayer<S> {
    event_filter: Box<dyn Fn(&Metadata) -> EventFilter + Send + Sync>,
    event_mapper: Option<EventMapper<S>>,

    span_filter: Box<dyn Fn(&Metadata) -> bool + Send + Sync>,

    with_span_attributes: bool,
}

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

    /// Enable every parent span's attributes to be sent along with own event's attributes.
    ///
    /// Note that the root span is considered a [transaction][sentry_core::protocol::Transaction]
    /// so its context will only be grabbed only if you set the transaction to be sampled.
    /// The most straightforward way to do this is to set
    /// the [traces_sample_rate][sentry_core::ClientOptions::traces_sample_rate] to `1.0`
    /// while configuring your sentry client.
    #[must_use]
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }
}

impl<S> Default for SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn default() -> Self {
        Self {
            event_filter: Box::new(default_event_filter),
            event_mapper: None,

            span_filter: Box::new(default_span_filter),

            with_span_attributes: false,
        }
    }
}

#[inline(always)]
fn record_fields<'a, K: AsRef<str> + Into<Cow<'a, str>>>(
    span: &TransactionOrSpan,
    data: BTreeMap<K, Value>,
) {
    match span {
        TransactionOrSpan::Span(span) => {
            let mut span = span.data();
            for (key, value) in data {
                if let Some(stripped_key) = key.as_ref().strip_prefix(TAGS_PREFIX) {
                    match value {
                        Value::Bool(value) => {
                            span.set_tag(stripped_key.to_owned(), value.to_string())
                        }
                        Value::Number(value) => {
                            span.set_tag(stripped_key.to_owned(), value.to_string())
                        }
                        Value::String(value) => span.set_tag(stripped_key.to_owned(), value),
                        _ => span.set_data(key.into().into_owned(), value),
                    }
                } else {
                    span.set_data(key.into().into_owned(), value);
                }
            }
        }
        TransactionOrSpan::Transaction(transaction) => {
            let mut transaction = transaction.data();
            for (key, value) in data {
                if let Some(stripped_key) = key.as_ref().strip_prefix(TAGS_PREFIX) {
                    match value {
                        Value::Bool(value) => {
                            transaction.set_tag(stripped_key.into(), value.to_string())
                        }
                        Value::Number(value) => {
                            transaction.set_tag(stripped_key.into(), value.to_string())
                        }
                        Value::String(value) => transaction.set_tag(stripped_key.into(), value),
                        _ => transaction.set_data(key.into(), value),
                    }
                } else {
                    transaction.set_data(key.into(), value);
                }
            }
        }
    }
}

/// Data that is attached to the tracing Spans `extensions`, in order to
/// `finish` the corresponding sentry span `on_close`, and re-set its parent as
/// the *current* span.
pub(super) struct SentrySpanData {
    pub(super) sentry_span: TransactionOrSpan,
    parent_sentry_span: Option<TransactionOrSpan>,
    hub: Arc<sentry_core::Hub>,
}

impl<S> Layer<S> for SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        let items = match &self.event_mapper {
            Some(mapper) => mapper(event, ctx),
            None => {
                let span_ctx = self.with_span_attributes.then_some(ctx);
                let filter = (self.event_filter)(event.metadata());
                let mut items = vec![];
                if filter.contains(EventFilter::Breadcrumb) {
                    items.push(EventMapping::Breadcrumb(breadcrumb_from_event(
                        event,
                        span_ctx.as_ref(),
                    )));
                }
                if filter.contains(EventFilter::Event) {
                    items.push(EventMapping::Event(event_from_event(
                        event,
                        span_ctx.as_ref(),
                    )));
                }
                #[cfg(feature = "logs")]
                if filter.contains(EventFilter::Log) {
                    items.push(EventMapping::Log(log_from_event(event, span_ctx.as_ref())));
                }
                EventMapping::Combined(CombinedEventMapping(items))
            }
        };
        let items = CombinedEventMapping::from(items);

        for item in items.0 {
            match item {
                EventMapping::Ignore => (),
                EventMapping::Breadcrumb(breadcrumb) => sentry_core::add_breadcrumb(breadcrumb),
                EventMapping::Event(event) => {
                    sentry_core::capture_event(event);
                }
                #[cfg(feature = "logs")]
                EventMapping::Log(log) => sentry_core::Hub::with_active(|hub| hub.capture_log(log)),
                EventMapping::Combined(_) => {
                    sentry_core::sentry_debug!(
                        "[SentryLayer] found nested CombinedEventMapping, ignoring"
                    )
                }
            }
        }
    }

    /// When a new Span gets created, run the filter and start a new sentry span
    /// if it passes, setting it as the *current* sentry span.
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        if !(self.span_filter)(span.metadata()) {
            return;
        }

        let (data, sentry_name, sentry_op, sentry_trace) = extract_span_data(attrs);
        let sentry_name = sentry_name.as_deref().unwrap_or_else(|| span.name());
        let sentry_op =
            sentry_op.unwrap_or_else(|| format!("{}::{}", span.metadata().target(), span.name()));

        let hub = sentry_core::Hub::current();
        let parent_sentry_span = hub.configure_scope(|scope| scope.get_span());

        let mut sentry_span: sentry_core::TransactionOrSpan = match &parent_sentry_span {
            Some(parent) => parent.start_child(&sentry_op, sentry_name).into(),
            None => {
                let ctx = if let Some(trace_header) = sentry_trace {
                    sentry_core::TransactionContext::continue_from_headers(
                        sentry_name,
                        &sentry_op,
                        [("sentry-trace", trace_header.as_str())],
                    )
                } else {
                    sentry_core::TransactionContext::new(sentry_name, &sentry_op)
                };

                let tx = sentry_core::start_transaction(ctx);
                tx.set_origin("auto.tracing");
                tx.into()
            }
        };
        // Add the data from the original span to the sentry span.
        // This comes from typically the `fields` in `tracing::instrument`.
        record_fields(&sentry_span, data);

        set_default_attributes(&mut sentry_span, span.metadata());

        let mut extensions = span.extensions_mut();
        extensions.insert(SentrySpanData {
            sentry_span,
            parent_sentry_span,
            hub,
        });
    }

    /// Sets entered span as *current* sentry span. A tracing span can be
    /// entered and existed multiple times, for example, when using a `tracing::Instrumented` future.
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        let extensions = span.extensions();
        if let Some(data) = extensions.get::<SentrySpanData>() {
            let guard = sentry_core::HubSwitchGuard::new(data.hub.clone());
            SPAN_GUARDS.with(|guards| {
                guards.borrow_mut().insert(id.clone(), guard);
            });
            data.hub.configure_scope(|scope| {
                scope.set_span(Some(data.sentry_span.clone()));
            });
        }
    }

    /// Set exited span's parent as *current* sentry span.
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        let extensions = span.extensions();
        if let Some(data) = extensions.get::<SentrySpanData>() {
            data.hub.configure_scope(|scope| {
                scope.set_span(data.parent_sentry_span.clone());
            });
        }
        SPAN_GUARDS.with(|guards| {
            guards.borrow_mut().remove(id);
        });
    }

    /// When a span gets closed, finish the underlying sentry span, and set back
    /// its parent as the *current* sentry span.
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        SPAN_GUARDS.with(|guards| {
            guards.borrow_mut().remove(&id);
        });

        let span = match ctx.span(&id) {
            Some(span) => span,
            None => return,
        };

        let mut extensions = span.extensions_mut();
        let SentrySpanData { sentry_span, .. } = match extensions.remove::<SentrySpanData>() {
            Some(data) => data,
            None => return,
        };

        sentry_span.finish();
    }

    /// Implement the writing of extra data to span
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let span = match ctx.span(span) {
            Some(s) => s,
            _ => return,
        };

        let mut extensions = span.extensions_mut();
        let span = match extensions.get_mut::<SentrySpanData>() {
            Some(t) => &t.sentry_span,
            _ => return,
        };

        let mut data = FieldVisitor::default();
        values.record(&mut data);

        let sentry_name = data
            .json_values
            .remove(SENTRY_NAME_FIELD)
            .and_then(|v| match v {
                Value::String(s) => Some(s),
                _ => None,
            });

        let sentry_op = data
            .json_values
            .remove(SENTRY_OP_FIELD)
            .and_then(|v| match v {
                Value::String(s) => Some(s),
                _ => None,
            });

        // `sentry.trace` cannot be applied retroactively
        data.json_values.remove(SENTRY_TRACE_FIELD);

        if let Some(name) = sentry_name {
            span.set_name(&name);
        }
        if let Some(op) = sentry_op {
            span.set_op(&op);
        }

        record_fields(span, data.json_values);
    }
}

fn set_default_attributes(span: &mut TransactionOrSpan, metadata: &Metadata<'_>) {
    span.set_data("sentry.tracing.target", metadata.target().into());

    if let Some(module) = metadata.module_path() {
        span.set_data("code.module.name", module.into());
    }

    if let Some(file) = metadata.file() {
        span.set_data("code.file.path", file.into());
    }

    if let Some(line) = metadata.line() {
        span.set_data("code.line.number", line.into());
    }
}

/// Creates a default Sentry layer
pub fn layer<S>() -> SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    Default::default()
}

/// Extracts the attributes from a span,
/// returning the values of SENTRY_NAME_FIELD, SENTRY_OP_FIELD, SENTRY_TRACE_FIELD separately
fn extract_span_data(
    attrs: &span::Attributes,
) -> (
    BTreeMap<&'static str, Value>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut json_values = VISITOR_BUFFER.with_borrow_mut(|debug_buffer| {
        let mut visitor = SpanFieldVisitor {
            debug_buffer,
            json_values: Default::default(),
        };
        attrs.record(&mut visitor);
        visitor.json_values
    });

    let name = json_values.remove(SENTRY_NAME_FIELD).and_then(|v| match v {
        Value::String(s) => Some(s),
        _ => None,
    });

    let op = json_values.remove(SENTRY_OP_FIELD).and_then(|v| match v {
        Value::String(s) => Some(s),
        _ => None,
    });

    let sentry_trace = json_values
        .remove(SENTRY_TRACE_FIELD)
        .and_then(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        });

    (json_values, name, op, sentry_trace)
}

thread_local! {
    static VISITOR_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
    /// Hub switch guards keyed by span ID. Stored in thread-local so guards are
    /// always dropped on the same thread where they were created.
    static SPAN_GUARDS: RefCell<HashMap<span::Id, sentry_core::HubSwitchGuard>> =
        RefCell::new(HashMap::new());
}

/// Records all span fields into a `BTreeMap`, reusing a mutable `String` as buffer.
struct SpanFieldVisitor<'s> {
    debug_buffer: &'s mut String,
    json_values: BTreeMap<&'static str, Value>,
}

impl SpanFieldVisitor<'_> {
    fn record<T: Into<Value>>(&mut self, field: &Field, value: T) {
        self.json_values.insert(field.name(), value.into());
    }
}

impl Visit for SpanFieldVisitor<'_> {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record(field, value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record(field, value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        self.debug_buffer.reserve(128);
        write!(self.debug_buffer, "{value:?}").unwrap();
        self.json_values
            .insert(field.name(), self.debug_buffer.as_str().into());
        self.debug_buffer.clear();
    }
}
