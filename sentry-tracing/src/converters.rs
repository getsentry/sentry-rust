use std::collections::BTreeMap;
use std::error::Error;

use sentry_core::protocol::{Event, Exception, Value};
use sentry_core::{event_from_error, Breadcrumb, Level};
use tracing_core::field::{Field, Visit};
use tracing_core::{span, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// Converts a [`tracing_core::Level`] to a Sentry [`Level`]
fn convert_tracing_level(level: &tracing_core::Level) -> Level {
    match level {
        &tracing_core::Level::TRACE | &tracing_core::Level::DEBUG => Level::Debug,
        &tracing_core::Level::INFO => Level::Info,
        &tracing_core::Level::WARN => Level::Warning,
        &tracing_core::Level::ERROR => Level::Error,
    }
}

/// Extracts the message and metadata from an event
fn extract_event_data(event: &tracing_core::Event) -> (Option<String>, FieldVisitor) {
    // Find message of the event, if any
    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);
    let message = visitor
        .json_values
        .remove("message")
        .and_then(|v| v.as_str().map(|s| s.to_owned()));

    (message, visitor)
}

/// Extracts the message and metadata from a span
pub(crate) fn extract_span_data(
    attrs: &span::Attributes,
) -> (Option<String>, BTreeMap<String, Value>) {
    let mut data = FieldVisitor::default();
    attrs.record(&mut data);

    // Find message of the span, if any
    let message = data
        .json_values
        .remove("message")
        .and_then(|v| v.as_str().map(|s| s.to_owned()));

    (message, data.json_values)
}

/// Records all fields of [`tracing_core::Event`] for easy access
#[derive(Default)]
pub(crate) struct FieldVisitor {
    pub json_values: BTreeMap<String, Value>,
    pub exceptions: Vec<Exception>,
}

impl FieldVisitor {
    fn record<T: Into<Value>>(&mut self, field: &Field, value: T) {
        self.json_values
            .insert(field.name().to_owned(), value.into());
    }
}

impl Visit for FieldVisitor {
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

    fn record_error(&mut self, _field: &Field, value: &(dyn Error + 'static)) {
        let event = event_from_error(value);
        for exception in event.exception {
            self.exceptions.push(exception);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record(field, format!("{:?}", value));
    }
}

/// Creates a [`Breadcrumb`] from a given [`tracing_core::Event`]
pub fn breadcrumb_from_event(event: &tracing_core::Event) -> Breadcrumb {
    let (message, visitor) = extract_event_data(event);
    Breadcrumb {
        category: Some(event.metadata().target().to_owned()),
        ty: "log".into(),
        level: convert_tracing_level(event.metadata().level()),
        message,
        data: visitor.json_values,
        ..Default::default()
    }
}

/// Creates an [`Event`] from a given [`tracing_core::Event`]
pub fn event_from_event<S>(event: &tracing_core::Event, _ctx: Context<S>) -> Event<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (message, visitor) = extract_event_data(event);

    Event {
        logger: Some(event.metadata().target().to_owned()),
        level: convert_tracing_level(event.metadata().level()),
        message,
        extra: visitor.json_values,
        ..Default::default()
    }
}

/// Creates an exception [`Event`] from a given [`tracing_core::Event`]
pub fn exception_from_event<S>(event: &tracing_core::Event, _ctx: Context<S>) -> Event<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    // TODO: Exception records in Sentry need a valid type, value and full stack trace to support
    // proper grouping and issue metadata generation. tracing_core::Record does not contain sufficient
    // information for this. However, it may contain a serialized error which we can parse to emit
    // an exception record.
    const DEFAULT_ERROR_VALUE_KEY: &str = "error";

    let (message, mut visitor) = extract_event_data(event);

    let exception = if visitor.exceptions.is_empty() {
        let ty = event.metadata().name().into();
        let value = visitor
            .json_values
            .remove(DEFAULT_ERROR_VALUE_KEY)
            .map(|v| v.as_str().map(|s| s.to_owned()))
            .flatten()
            .or_else(|| message.clone());
        vec![Exception {
            ty,
            value,
            module: event.metadata().module_path().map(String::from),
            stacktrace: sentry_backtrace::current_stacktrace(),
            ..Default::default()
        }]
    } else {
        visitor.exceptions
    }
    .into();
    Event {
        logger: Some(event.metadata().target().to_owned()),
        level: convert_tracing_level(event.metadata().level()),
        message,
        extra: visitor.json_values,
        exception,
        ..Default::default()
    }
}
