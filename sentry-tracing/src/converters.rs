use std::collections::BTreeMap;
use std::error::Error;

use sentry_core::protocol::{Event, Exception, Mechanism, Value};
#[cfg(feature = "logs")]
use sentry_core::protocol::{Log, LogAttribute, LogLevel};
use sentry_core::{event_from_error, Breadcrumb, Level, TransactionOrSpan};
#[cfg(feature = "logs")]
use std::time::SystemTime;
use tracing_core::field::{Field, Visit};
use tracing_core::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::layer::SentrySpanData;
use crate::TAGS_PREFIX;

/// Converts a [`tracing_core::Level`] to a Sentry [`Level`], used for events and breadcrumbs.
fn level_to_sentry_level(level: &tracing_core::Level) -> Level {
    match *level {
        tracing_core::Level::TRACE | tracing_core::Level::DEBUG => Level::Debug,
        tracing_core::Level::INFO => Level::Info,
        tracing_core::Level::WARN => Level::Warning,
        tracing_core::Level::ERROR => Level::Error,
    }
}

/// Converts a [`tracing_core::Level`] to a Sentry [`LogLevel`], used for logs.
#[cfg(feature = "logs")]
fn level_to_log_level(level: &tracing_core::Level) -> LogLevel {
    match *level {
        tracing_core::Level::TRACE => LogLevel::Trace,
        tracing_core::Level::DEBUG => LogLevel::Debug,
        tracing_core::Level::INFO => LogLevel::Info,
        tracing_core::Level::WARN => LogLevel::Warn,
        tracing_core::Level::ERROR => LogLevel::Error,
    }
}

/// Converts a [`tracing_core::Level`] to the corresponding Sentry [`Exception::ty`] entry.
#[allow(unused)]
fn level_to_exception_type(level: &tracing_core::Level) -> &'static str {
    match *level {
        tracing_core::Level::TRACE => "tracing::trace!",
        tracing_core::Level::DEBUG => "tracing::debug!",
        tracing_core::Level::INFO => "tracing::info!",
        tracing_core::Level::WARN => "tracing::warn!",
        tracing_core::Level::ERROR => "tracing::error!",
    }
}

/// Extracts the message and metadata from an event.
fn extract_event_data(
    event: &tracing_core::Event,
    store_errors_in_values: bool,
) -> (Option<String>, FieldVisitor) {
    // Find message of the event, if any
    let mut visitor = FieldVisitor {
        store_errors_in_values,
        ..Default::default()
    };
    event.record(&mut visitor);
    let message = visitor
        .json_values
        .remove("message")
        // When #[instrument(err)] is used the event does not have a message attached to it.
        // the error message is attached to the field "error".
        .or_else(|| visitor.json_values.remove("error"))
        .and_then(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        });

    (message, visitor)
}

/// Extracts the message and metadata from an event, including the data in the current span.
fn extract_event_data_with_context<S>(
    event: &tracing_core::Event,
    ctx: Option<&Context<S>>,
    store_errors_in_values: bool,
) -> (Option<String>, FieldVisitor)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (message, mut visitor) = extract_event_data(event, store_errors_in_values);

    // Add the context fields of every parent span.
    let current_span = ctx.as_ref().and_then(|ctx| {
        event
            .parent()
            .and_then(|id| ctx.span(id))
            .or_else(|| ctx.lookup_current())
    });
    if let Some(span) = current_span {
        for span in span.scope() {
            let name = span.name();
            let ext = span.extensions();

            if let Some(span_data) = ext.get::<SentrySpanData>() {
                match &span_data.sentry_span {
                    TransactionOrSpan::Span(span) => {
                        for (key, value) in span.data().iter() {
                            if key != "message" {
                                let key = format!("{name}:{key}");
                                visitor.json_values.insert(key, value.clone());
                            }
                        }
                    }
                    TransactionOrSpan::Transaction(transaction) => {
                        for (key, value) in transaction.data().iter() {
                            if key != "message" {
                                let key = format!("{name}:{key}");
                                visitor.json_values.insert(key, value.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    (message, visitor)
}

/// Records the fields of a [`tracing_core::Event`].
#[derive(Default)]
pub(crate) struct FieldVisitor {
    pub(crate) json_values: BTreeMap<String, Value>,
    pub(crate) exceptions: Vec<Exception>,
    /// If `true`, stringify and store errors in `self.json_values` under the original field name
    /// else (default), convert to `Exception`s and store in `self.exceptions`.
    store_errors_in_values: bool,
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

    fn record_error(&mut self, field: &Field, value: &(dyn Error + 'static)) {
        let event = event_from_error(value);
        if self.store_errors_in_values {
            let error_chain = event
                .exception
                .iter()
                .rev()
                .filter_map(|x| x.value.as_ref().map(|v| format!("{}: {}", x.ty, *v)))
                .collect::<Vec<String>>();
            self.record(field, error_chain);
        } else {
            for exception in event.exception {
                self.exceptions.push(exception);
            }
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record(field, format!("{value:?}"));
    }
}

/// Creates a [`Breadcrumb`] from a given [`tracing_core::Event`].
pub fn breadcrumb_from_event<'context, S>(
    event: &tracing_core::Event,
    ctx: impl Into<Option<&'context Context<'context, S>>>,
) -> Breadcrumb
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (message, visitor) = extract_event_data_with_context(event, ctx.into(), true);

    Breadcrumb {
        category: Some(event.metadata().target().to_owned()),
        ty: "log".into(),
        level: level_to_sentry_level(event.metadata().level()),
        message,
        data: visitor.json_values,
        ..Default::default()
    }
}

/// Convert `tracing` fields to the corresponding Sentry tags, removing them from `fields`.
fn extract_and_remove_tags(fields: &mut BTreeMap<String, Value>) -> BTreeMap<String, String> {
    let mut tags = BTreeMap::new();

    fields.retain(|key, value| {
        let Some(key) = key.strip_prefix(TAGS_PREFIX) else {
            return true;
        };
        let string = match value {
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => std::mem::take(s),
            // remove null entries since empty tags are not allowed
            Value::Null => return false,
            // keep entries that cannot be represented as simple string
            Value::Array(_) | Value::Object(_) => return true,
        };

        tags.insert(key.to_owned(), string);

        false
    });

    tags
}

/// Create Sentry Contexts out of the `tracing` event and fields.
fn contexts_from_event(
    event: &tracing_core::Event,
    fields: BTreeMap<String, Value>,
) -> BTreeMap<String, sentry_core::protocol::Context> {
    let event_meta = event.metadata();
    let mut location_map = BTreeMap::new();
    if let Some(module_path) = event_meta.module_path() {
        location_map.insert("module_path".to_string(), module_path.into());
    }
    if let Some(file) = event_meta.file() {
        location_map.insert("file".to_string(), file.into());
    }
    if let Some(line) = event_meta.line() {
        location_map.insert("line".to_string(), line.into());
    }

    let mut context = BTreeMap::new();
    if !fields.is_empty() {
        context.insert(
            "Rust Tracing Fields".to_string(),
            sentry_core::protocol::Context::Other(fields),
        );
    }
    if !location_map.is_empty() {
        context.insert(
            "Rust Tracing Location".to_string(),
            sentry_core::protocol::Context::Other(location_map),
        );
    }
    context
}

/// Creates an [`Event`] (possibly carrying exceptions) from a given [`tracing_core::Event`].
pub fn event_from_event<'context, S>(
    event: &tracing_core::Event,
    ctx: impl Into<Option<&'context Context<'context, S>>>,
) -> Event<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    // Exception records in Sentry need a valid type, value and full stack trace to support
    // proper grouping and issue metadata generation. tracing_core::Record does not contain sufficient
    // information for this. However, it may contain a serialized error which we can parse to emit
    // an exception record.
    #[allow(unused_mut)]
    let (mut message, visitor) = extract_event_data_with_context(event, ctx.into(), false);
    let FieldVisitor {
        mut exceptions,
        mut json_values,
        store_errors_in_values: _,
    } = visitor;

    // If there are a message, an exception, and we are capturing stack traces, then add the message
    // as synthetic wrapper around the exception to support proper grouping. The stack trace to
    // attach is the current one, since it points to the place where the exception is captured.
    // We should only do this if we're capturing stack traces, otherwise the issue title will be `<unknown>`
    // as Sentry will attempt to use missing stack trace to determine the title.
    #[cfg(feature = "backtrace")]
    if !exceptions.is_empty() && message.is_some() {
        if let Some(client) = sentry_core::Hub::current().client() {
            if client.options().attach_stacktrace {
                let thread = sentry_backtrace::current_thread(true);
                let exception = Exception {
                    ty: level_to_exception_type(event.metadata().level()).to_owned(),
                    value: message.take(),
                    module: event.metadata().module_path().map(str::to_owned),
                    stacktrace: thread.stacktrace,
                    raw_stacktrace: thread.raw_stacktrace,
                    thread_id: thread.id,
                    mechanism: Some(Mechanism {
                        synthetic: Some(true),
                        ..Mechanism::default()
                    }),
                };
                exceptions.push(exception)
            }
        }
    }

    if let Some(exception) = exceptions.last_mut() {
        "tracing".clone_into(
            &mut exception
                .mechanism
                .get_or_insert_with(Mechanism::default)
                .ty,
        );
    }

    Event {
        logger: Some(event.metadata().target().to_owned()),
        level: level_to_sentry_level(event.metadata().level()),
        message,
        exception: exceptions.into(),
        tags: extract_and_remove_tags(&mut json_values),
        contexts: contexts_from_event(event, json_values),
        ..Default::default()
    }
}

/// Creates a [`Log`] from a given [`tracing_core::Event`]
#[cfg(feature = "logs")]
pub fn log_from_event<'context, S>(
    event: &tracing_core::Event,
    ctx: impl Into<Option<&'context Context<'context, S>>>,
) -> Log
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (message, visitor) = extract_event_data_with_context(event, ctx.into(), true);

    let mut attributes: BTreeMap<String, LogAttribute> = visitor
        .json_values
        .into_iter()
        .map(|(key, val)| (key, val.into()))
        .collect();

    let event_meta = event.metadata();
    if let Some(module_path) = event_meta.module_path() {
        attributes.insert("code.module.name".to_owned(), module_path.into());
    }
    if let Some(file) = event_meta.file() {
        attributes.insert("code.file.path".to_owned(), file.into());
    }
    if let Some(line) = event_meta.line() {
        attributes.insert("code.line.number".to_owned(), line.into());
    }

    attributes.insert("sentry.origin".to_owned(), "auto.tracing".into());

    Log {
        level: level_to_log_level(event.metadata().level()),
        body: message.unwrap_or_default(),
        trace_id: None,
        timestamp: SystemTime::now(),
        severity_number: None,
        attributes,
    }
}
