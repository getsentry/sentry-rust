use sentry_core::protocol::{Event, Value};
#[cfg(feature = "logs")]
use sentry_core::protocol::{Log, LogAttribute, LogLevel};
use sentry_core::{Breadcrumb, Level};
use std::collections::BTreeMap;
#[cfg(feature = "logs")]
use std::time::SystemTime;

/// Converts a [`log::Level`] to a Sentry [`Level`], used for [`Event`] and [`Breadcrumb`].
pub fn convert_log_level(level: log::Level) -> Level {
    match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warning,
        log::Level::Info => Level::Info,
        log::Level::Debug | log::Level::Trace => Level::Debug,
    }
}

/// Converts a [`log::Level`] to a Sentry [`LogLevel`], used for [`Log`].
#[cfg(feature = "logs")]
pub fn convert_log_level_to_sentry_log_level(level: log::Level) -> LogLevel {
    match level {
        log::Level::Error => LogLevel::Error,
        log::Level::Warn => LogLevel::Warn,
        log::Level::Info => LogLevel::Info,
        log::Level::Debug => LogLevel::Debug,
        log::Level::Trace => LogLevel::Trace,
    }
}

/// Visitor to extract key-value pairs from log records
#[derive(Default)]
struct AttributeVisitor {
    json_values: BTreeMap<String, Value>,
}

impl AttributeVisitor {
    fn record<T: Into<Value>>(&mut self, key: &str, value: T) {
        self.json_values.insert(key.to_owned(), value.into());
    }
}

impl log::kv::VisitSource<'_> for AttributeVisitor {
    fn visit_pair(
        &mut self,
        key: log::kv::Key,
        value: log::kv::Value,
    ) -> Result<(), log::kv::Error> {
        let key = key.as_str();

        if let Some(value) = value.to_borrowed_str() {
            self.record(key, value);
        } else if let Some(value) = value.to_u64() {
            self.record(key, value);
        } else if let Some(value) = value.to_f64() {
            self.record(key, value);
        } else if let Some(value) = value.to_bool() {
            self.record(key, value);
        } else {
            self.record(key, format!("{value:?}"));
        };

        Ok(())
    }
}

fn extract_record_attributes(record: &log::Record<'_>) -> AttributeVisitor {
    let mut visitor = AttributeVisitor::default();
    let _ = record.key_values().visit(&mut visitor);
    visitor
}

/// Creates a [`Breadcrumb`] from a given [`log::Record`].
pub fn breadcrumb_from_record(record: &log::Record<'_>) -> Breadcrumb {
    let visitor = extract_record_attributes(record);

    Breadcrumb {
        ty: "log".into(),
        level: convert_log_level(record.level()),
        category: Some(record.target().into()),
        message: Some(record.args().to_string()),
        data: visitor.json_values,
        ..Default::default()
    }
}

/// Creates an [`Event`] from a given [`log::Record`].
pub fn event_from_record(record: &log::Record<'_>) -> Event<'static> {
    let visitor = extract_record_attributes(record);
    let attributes = visitor.json_values;

    let mut contexts = BTreeMap::new();

    let mut metadata_map = BTreeMap::new();
    metadata_map.insert("logger.target".into(), record.target().into());
    if let Some(module_path) = record.module_path() {
        metadata_map.insert("logger.module_path".into(), module_path.into());
    }
    if let Some(file) = record.file() {
        metadata_map.insert("logger.file".into(), file.into());
    }
    if let Some(line) = record.line() {
        metadata_map.insert("logger.line".into(), line.into());
    }
    contexts.insert(
        "Rust Log Metadata".to_string(),
        sentry_core::protocol::Context::Other(metadata_map),
    );

    if !attributes.is_empty() {
        contexts.insert(
            "Rust Log Attributes".to_string(),
            sentry_core::protocol::Context::Other(attributes),
        );
    }

    Event {
        logger: Some(record.target().into()),
        level: convert_log_level(record.level()),
        message: Some(record.args().to_string()),
        contexts,
        ..Default::default()
    }
}

/// Creates an exception [`Event`] from a given [`log::Record`].
pub fn exception_from_record(record: &log::Record<'_>) -> Event<'static> {
    // TODO: Exception records in Sentry need a valid type, value and full stack trace to support
    // proper grouping and issue metadata generation. log::Record does not contain sufficient
    // information for this. However, it may contain a serialized error which we can parse to emit
    // an exception record.
    event_from_record(record)
}

/// Creates a [`Log`] from a given [`log::Record`].
#[cfg(feature = "logs")]
pub fn log_from_record(record: &log::Record<'_>) -> Log {
    let visitor = extract_record_attributes(record);

    let mut attributes: BTreeMap<String, LogAttribute> = visitor
        .json_values
        .into_iter()
        .map(|(key, val)| (key, val.into()))
        .collect();

    attributes.insert("logger.target".into(), record.target().into());
    if let Some(module_path) = record.module_path() {
        attributes.insert("logger.module_path".into(), module_path.into());
    }
    if let Some(file) = record.file() {
        attributes.insert("logger.file".into(), file.into());
    }
    if let Some(line) = record.line() {
        attributes.insert("logger.line".into(), line.into());
    }

    attributes.insert("sentry.origin".into(), "auto.logger.log".into());

    Log {
        level: convert_log_level_to_sentry_log_level(record.level()),
        body: format!("{}", record.args()),
        trace_id: None,
        timestamp: SystemTime::now(),
        severity_number: None,
        attributes,
    }
}
