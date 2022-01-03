use std::{
    collections::BTreeMap,
    time::{Instant, SystemTime},
};

use sentry_core::{
    protocol::{self, Breadcrumb, TraceContext, TraceId, Transaction, Value},
    types::Uuid,
    Envelope, Hub,
};
use tracing_core::{span, Event, Level, Metadata, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::{LookupSpan, SpanRef},
};

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
pub fn default_event_filter(metadata: &Metadata) -> EventFilter {
    match metadata.level() {
        &Level::ERROR => EventFilter::Exception,
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

/// The default span mapper.
///
/// By default, a new empty span is created with the `op`
/// field set to the name of the span, with the `trace_id`
/// copied from the parent span if any
pub fn default_span_mapper<S>(
    span: &SpanRef<S>,
    parent: Option<&protocol::Span>,
    attrs: &span::Attributes,
) -> protocol::Span
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (description, data) = extract_span_data(attrs);

    let trace_id = parent
        .map(|parent| parent.trace_id)
        .unwrap_or_else(TraceId::default);

    protocol::Span {
        trace_id,
        op: Some(span.name().into()),
        description,
        data,
        ..protocol::Span::default()
    }
}

/// The default span on_close hook.
///
/// By default, this sets the end timestamp of the span,
/// and creates `busy` and `idle` data fields from the timing data
pub fn default_span_on_close(span: &mut protocol::Span, timings: Timings) {
    span.data
        .insert(String::from("busy"), Value::Number(timings.busy.into()));

    span.data
        .insert(String::from("idle"), Value::Number(timings.idle.into()));

    span.timestamp = Some(timings.end_time.into());
}

/// The default transaction mapper.
///
/// By default, this creates a transaction from a root span
/// containing all of its children spans
pub fn default_transaction_mapper<S>(
    sentry_span: protocol::Span,
    tracing_span: &SpanRef<S>,
    spans: Vec<protocol::Span>,
    timings: Timings,
) -> Transaction<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let mut contexts = BTreeMap::new();

    contexts.insert(
        String::from("trace"),
        protocol::Context::Trace(Box::new(TraceContext {
            span_id: sentry_span.span_id,
            trace_id: sentry_span.trace_id,
            parent_span_id: sentry_span.parent_span_id,
            op: sentry_span.op,
            description: sentry_span.description,
            status: sentry_span.status,
        })),
    );

    Transaction {
        event_id: Uuid::new_v4(),
        name: Some(tracing_span.name().into()),
        start_timestamp: timings.start_time.into(),
        timestamp: Some(timings.end_time.into()),
        spans,
        contexts,
        ..Transaction::default()
    }
}

type EventMapper<S> = Box<dyn Fn(&Event, Context<'_, S>) -> EventMapping + Send + Sync>;

type SpanMapper<S> = Box<
    dyn Fn(&SpanRef<S>, Option<&protocol::Span>, &span::Attributes) -> protocol::Span + Send + Sync,
>;

type SpanOnClose = Box<dyn Fn(&mut protocol::Span, Timings) + Send + Sync>;

type TransactionMapper<S> = Box<
    dyn Fn(protocol::Span, &SpanRef<S>, Vec<protocol::Span>, Timings) -> Transaction<'static>
        + Send
        + Sync,
>;

/// Provides a tracing layer that dispatches events to sentry
pub struct SentryLayer<S> {
    event_filter: Box<dyn Fn(&Metadata) -> EventFilter + Send + Sync>,
    event_mapper: Option<EventMapper<S>>,

    span_filter: Box<dyn Fn(&Metadata) -> bool + Send + Sync>,
    span_mapper: SpanMapper<S>,
    span_on_close: SpanOnClose,
    transaction_mapper: TransactionMapper<S>,
}

impl<S> SentryLayer<S> {
    /// Sets a custom event filter function.
    ///
    /// The filter classifies how sentry should handle [`Event`]s based
    /// on their [`Metadata`].
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
    pub fn span_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Metadata) -> bool + Send + Sync + 'static,
    {
        self.span_filter = Box::new(filter);
        self
    }

    /// Sets a custom span mapper function.
    ///
    /// The mapper is responsible for creating [`protocol::Span`]s from
    /// [`tracing::Span`]s.
    pub fn span_mapper<F>(mut self, mapper: F) -> Self
    where
        F: Fn(&SpanRef<S>, Option<&protocol::Span>, &span::Attributes) -> protocol::Span
            + Send
            + Sync
            + 'static,
    {
        self.span_mapper = Box::new(mapper);
        self
    }

    /// Sets a custom span `on_close` hook.
    ///
    /// The hook is called with [`Timings`] information when a [`tracing::Span`]
    /// is closed, and can mutate the associated [`protocol::Span`] accordingly.
    pub fn span_on_close<F>(mut self, on_close: F) -> Self
    where
        F: Fn(&mut protocol::Span, Timings) + Send + Sync + 'static,
    {
        self.span_on_close = Box::new(on_close);
        self
    }

    /// Sets a custom transaction mapper function.
    ///
    /// The mapper is responsible for creating [`Transaction`]s from
    /// [`tracing::Span`]s.
    pub fn transaction_mapper<F>(mut self, mapper: F) -> Self
    where
        F: Fn(protocol::Span, &SpanRef<S>, Vec<protocol::Span>, Timings) -> Transaction<'static>
            + Send
            + Sync
            + 'static,
    {
        self.transaction_mapper = Box::new(mapper);
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
            span_mapper: Box::new(default_span_mapper),
            span_on_close: Box::new(default_span_on_close),
            transaction_mapper: Box::new(default_transaction_mapper),
        }
    }
}

impl<S> Layer<S> for SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        let item = match &self.event_mapper {
            Some(mapper) => mapper(event, ctx),
            None => match (self.event_filter)(event.metadata()) {
                EventFilter::Ignore => EventMapping::Ignore,
                EventFilter::Breadcrumb => EventMapping::Breadcrumb(breadcrumb_from_event(event)),
                EventFilter::Event => EventMapping::Event(event_from_event(event, ctx)),
                EventFilter::Exception => EventMapping::Event(exception_from_event(event, ctx)),
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

    /// When a new Span gets created, run the filter and initialize the trace extension
    /// if it passes
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        if !(self.span_filter)(span.metadata()) {
            return;
        }

        let mut extensions = span.extensions_mut();
        if extensions.get_mut::<Trace>().is_none() {
            for parent in span.parent().into_iter().flat_map(|span| span.scope()) {
                let parent = parent.extensions();
                let parent = match parent.get::<Trace>() {
                    Some(trace) => trace,
                    None => continue,
                };

                let span = (self.span_mapper)(&span, Some(&parent.span), attrs);
                extensions.insert(Trace::new(span));
                return;
            }

            let span = (self.span_mapper)(&span, None, attrs);
            extensions.insert(Trace::new(span));
        }
    }

    /// From the tracing-subscriber implementation of span timings,
    /// keep track of when the span was last entered
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        let mut extensions = span.extensions_mut();
        if let Some(timings) = extensions.get_mut::<Trace>() {
            let now = Instant::now();
            timings.idle += (now - timings.last).as_nanos() as u64;
            timings.last = now;
        }
    }

    /// From the tracing-subscriber implementation of span timings,
    /// keep track of when the span was last exited
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        let mut extensions = span.extensions_mut();
        if let Some(timings) = extensions.get_mut::<Trace>() {
            let now = Instant::now();
            timings.busy += (now - timings.last).as_nanos() as u64;
            timings.last = now;
            timings.last_sys = SystemTime::now();
        }
    }

    /// When a span gets closed, if it has a trace extension either
    /// attach it to a parent span or submit it as a Transaction if
    /// it is a root of the span tree
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(&id) {
            Some(span) => span,
            None => return,
        };

        let mut extensions = span.extensions_mut();
        let mut trace = match extensions.remove::<Trace>() {
            Some(trace) => trace,
            None => return,
        };

        // Construct the timing data and call the on_close hook
        trace.idle += (Instant::now() - trace.last).as_nanos() as u64;

        let timings = Timings {
            start_time: trace.first,
            end_time: trace.last_sys,
            idle: trace.idle,
            busy: trace.busy,
        };

        (self.span_on_close)(&mut trace.span, timings);

        // Traverse the parents of this span to attach to the nearest one
        // that has tracing data (spans ignored by the span_filter do not)
        for parent in span.parent().into_iter().flat_map(|span| span.scope()) {
            let mut extensions = parent.extensions_mut();
            if let Some(parent) = extensions.get_mut::<Trace>() {
                parent.spans.extend(trace.spans);

                trace.span.parent_span_id = Some(parent.span.span_id);
                parent.spans.push(trace.span);
                return;
            }
        }

        // If no parent was found, consider this span a
        // transaction root and submit it to Sentry
        let span = &span;
        Hub::with_active(move |hub| {
            let client = match hub.client() {
                Some(client) => client,
                None => return,
            };

            if !client.sample_traces_should_send() {
                return;
            }

            let transaction = (self.transaction_mapper)(trace.span, span, trace.spans, timings);
            let envelope = Envelope::from(transaction);
            client.send_envelope(envelope);
        });
    }

    /// Implement the writing of extra data to span
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let span = match ctx.span(span) {
            Some(s) => s,
            _ => return,
        };

        let mut extensions_holder = span.extensions_mut();
        let trace = match extensions_holder.get_mut::<Trace>() {
            Some(t) => t,
            _ => return,
        };

        let mut data = BTreeMapRecorder::default();
        values.record(&mut data);

        for (key, value) in data.0 {
            trace.span.data.insert(key, value);
        }
    }
}

/// Timing informations for a given Span
#[derive(Clone, Copy, Debug)]
pub struct Timings {
    /// The time the span was first entered
    pub start_time: SystemTime,
    /// The time the span was last entered
    pub end_time: SystemTime,
    /// The total busy time for this span, in nanoseconds
    pub busy: u64,
    /// The total idle time for this span, in nanoseconds
    pub idle: u64,
}

/// Private internal state for a Span
///
/// Every Span that passes the `span_filter` has
/// an instance of this struct attached as an extension.
/// It is used to store transient informations while the
/// Span is being built such as the incomplete protocol::Span
/// as well as finished children Spans.
pub(crate) struct Trace {
    pub(crate) span: protocol::Span,
    spans: Vec<protocol::Span>,

    // From the tracing-subscriber implementation of span timings,
    // with additional SystemTime informations to reconstruct the UTC
    // times needed by Sentry
    idle: u64,
    busy: u64,
    last: Instant,
    first: SystemTime,
    last_sys: SystemTime,
}

impl Trace {
    fn new(span: protocol::Span) -> Self {
        Trace {
            span,
            spans: Vec::new(),

            idle: 0,
            busy: 0,
            last: Instant::now(),
            first: SystemTime::now(),
            last_sys: SystemTime::now(),
        }
    }
}

/// Creates a default Sentry layer
pub fn layer<S>() -> SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    Default::default()
}
