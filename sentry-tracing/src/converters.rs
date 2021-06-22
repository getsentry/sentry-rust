use std::collections::BTreeMap;

use sentry_core::protocol::{Event, Value};
use sentry_core::{Breadcrumb, Level};
use tracing_core::field::{Field, Visit};

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
pub fn extract_data(event: &tracing_core::Event) -> (Option<String>, BTreeMap<String, Value>) {
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

#[derive(Default)]
/// Records all fields of [`tracing_core::Event`] for easy access
struct BTreeMapRecorder(pub BTreeMap<String, Value>);

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
}

/// Creates a [`Breadcrumb`] from a given [`tracing_core::Event`]
pub fn breadcrumb_from_event(event: &tracing_core::Event) -> Breadcrumb {
    let (message, data) = extract_data(event);
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
pub fn event_from_event(event: &tracing_core::Event) -> Event<'static> {
    let (message, extra) = extract_data(event);
    Event {
        logger: Some(event.metadata().target().to_owned()),
        level: convert_tracing_level(event.metadata().level()),
        message,
        extra,
        ..Default::default()
    }
}

/// Creates an exception [`Event`] from a given [`tracing_core::Event`]
pub fn exception_from_event(event: &tracing_core::Event) -> Event<'static> {
    // TODO: Exception records in Sentry need a valid type, value and full stack trace to support
    // proper grouping and issue metadata generation. tracing_core::Record does not contain sufficient
    // information for this. However, it may contain a serialized error which we can parse to emit
    // an exception record.
    event_from_event(event)
}
