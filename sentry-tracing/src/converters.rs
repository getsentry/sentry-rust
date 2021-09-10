use std::collections::{BTreeMap, HashMap};
use std::error::Error;

use tracing_core::{
    field::{Field, Visit},
    span, Subscriber,
};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use sentry_core::protocol::{self, Event, Exception, TraceContext, Value};
use sentry_core::{Breadcrumb, Level};

use crate::Trace;
use std::iter::FromIterator;

/// Converts a [`tracing_core::Level`] to a Sentry [`Level`]
pub fn convert_tracing_level(level: &tracing_core::Level) -> Level {
    match level {
        &tracing_core::Level::TRACE | &tracing_core::Level::DEBUG => Level::Debug,
        &tracing_core::Level::INFO => Level::Info,
        &tracing_core::Level::WARN => Level::Warning,
        &tracing_core::Level::ERROR => Level::Error,
    }
}

/// Extracts the message and metadata from an event
pub fn extract_event_data(
    event: &tracing_core::Event,
) -> (Option<String>, BTreeMap<String, Value>) {
    // Find message of the event, if any
    let mut data = BTreeMapRecorder::default();
    event.record(&mut data);
    let message = data
        .0
        .remove("message")
        .map(|v| v.as_str().map(|s| s.to_owned()))
        .flatten();

    (message, data.0)
}

/// Extracts the message and metadata from a span
pub fn extract_span_data(attrs: &span::Attributes) -> (Option<String>, BTreeMap<String, Value>) {
    let mut data = BTreeMapRecorder::default();
    attrs.record(&mut data);

    // Find message of the span, if any
    let message = data
        .0
        .remove("message")
        .map(|v| v.as_str().map(|s| s.to_owned()))
        .flatten();

    (message, data.0)
}

#[derive(Default)]
/// Records all fields of [`tracing_core::Event`] for easy access
pub(crate) struct BTreeMapRecorder(pub BTreeMap<String, Value>);

impl BTreeMapRecorder {
    fn record<T: Into<Value>>(&mut self, field: &Field, value: T) {
        self.0.insert(field.name().to_owned(), value.into());
    }
}

impl Visit for BTreeMapRecorder {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record(field, format!("{:?}", value));
    }
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
    fn record_error(&mut self, field: &Field, value: &(dyn Error + 'static)) {
        let value: HashMap<String, Value> = {
            let mut h: HashMap<String, Value> = HashMap::new();
            h.insert(
                "ty".into(),
                std::any::type_name::<dyn Error>().to_string().into(),
            );
            h.insert("value".into(), value.to_string().into());
            h.insert(
                "backtrace".into(),
                value
                    .source()
                    .map(|bt| bt.to_string())
                    .unwrap_or_else(|| String::from("none"))
                    .into(),
            );
            h
        };
        let map = protocol::value::Map::from_iter(value.into_iter());
        self.record(field, Value::Object(map))
    }
}

/// Creates a [`Breadcrumb`] from a given [`tracing_core::Event`]
pub fn breadcrumb_from_event(event: &tracing_core::Event) -> Breadcrumb {
    let (message, data) = extract_event_data(event);
    Breadcrumb {
        category: Some(event.metadata().target().to_owned()),
        ty: "log".into(),
        level: convert_tracing_level(event.metadata().level()),
        message,
        data,
        ..Default::default()
    }
}

/// Creates an [`Event`] from a given [`tracing_core::Event`]
pub fn event_from_event<S>(event: &tracing_core::Event, ctx: Context<S>) -> Event<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (message, extra) = extract_event_data(event);

    let mut result = Event {
        logger: Some(event.metadata().target().to_owned()),
        level: convert_tracing_level(event.metadata().level()),
        message,
        extra,
        ..Default::default()
    };

    let parent = event
        .parent()
        .and_then(|id| ctx.span(id))
        .or_else(|| ctx.lookup_current());

    if let Some(parent) = parent {
        let extensions = parent.extensions();
        if let Some(trace) = extensions.get::<Trace>() {
            let context = protocol::Context::from(TraceContext {
                span_id: trace.span.span_id,
                trace_id: trace.span.trace_id,
                ..TraceContext::default()
            });

            result.contexts.insert(String::from("trace"), context);

            result.transaction = parent
                .parent()
                .into_iter()
                .flat_map(|span| span.scope())
                .last()
                .map(|root| root.name().into());
        }
    }

    result
}

/// Creates an exception [`Event`] from a given [`tracing_core::Event`]
pub fn exception_from_event<S>(event: &tracing_core::Event, ctx: Context<S>) -> Event<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    const DEFAULT_EXCEPTION_TYPE: &str = "Unknown";
    const DEFAULT_ERROR_VALUE_KEY: &str = "error";
    const DEFAULT_ERROR_TYPE_KEY: &str = "error_kind";

    // TODO: Exception records in Sentry need a valid type, value and full stack trace to support
    // proper grouping and issue metadata generation. tracing_core::Record does not contain sufficient
    // information for this. However, it may contain a serialized error which we can parse to emit
    // an exception record.
    let (message, mut extra) = extract_event_data(event);
    let ty = extra
        .remove(DEFAULT_ERROR_TYPE_KEY)
        .map(|v| v.as_str().map(|s| s.to_owned()))
        .flatten()
        .unwrap_or_else(|| String::from(DEFAULT_EXCEPTION_TYPE));
    let value = extra
        .remove(DEFAULT_ERROR_VALUE_KEY)
        .map(|v| v.as_str().map(|s| s.to_owned()))
        .flatten()
        .or_else(|| message.clone());
    let module = event.metadata().module_path().map(|s| s.to_string());
    let exception = Exception {
        ty,
        value,
        module,
        ..Default::default()
    };
    let mut result = Event {
        logger: Some(event.metadata().target().to_owned()),
        level: convert_tracing_level(event.metadata().level()),
        message,
        extra,
        exception: vec![exception].into(),
        ..Default::default()
    };

    let parent = event
        .parent()
        .and_then(|id| ctx.span(id))
        .or_else(|| ctx.lookup_current());
    if let Some(parent) = parent {
        let extensions = parent.extensions();
        if let Some(trace) = extensions.get::<Trace>() {
            let context = protocol::Context::from(TraceContext {
                span_id: trace.span.span_id,
                trace_id: trace.span.trace_id,
                ..TraceContext::default()
            });
            result.contexts.insert(String::from("trace"), context);
            result.transaction = parent
                .parent()
                .into_iter()
                .flat_map(|span| span.scope())
                .last()
                .map(|root| root.name().into());
        }
    }
    result
}
