use sentry_core::protocol::Event;
#[cfg(feature = "logs")]
use sentry_core::protocol::{Log, LogAttribute, LogLevel};
use sentry_core::{Breadcrumb, Level};
#[cfg(feature = "logs")]
use std::{collections::BTreeMap, time::SystemTime};

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

/// Creates a [`Breadcrumb`] from a given [`log::Record`].
pub fn breadcrumb_from_record(record: &log::Record<'_>) -> Breadcrumb {
    Breadcrumb {
        ty: "log".into(),
        level: convert_log_level(record.level()),
        category: Some(record.target().into()),
        message: Some(record.args().to_string()),
        ..Default::default()
    }
}

/// Creates an [`Event`] from a given [`log::Record`].
pub fn event_from_record(record: &log::Record<'_>) -> Event<'static> {
    Event {
        logger: Some(record.target().into()),
        level: convert_log_level(record.level()),
        message: Some(record.args().to_string()),
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
    let mut attributes: BTreeMap<String, LogAttribute> = BTreeMap::new();

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

    // TODO: support the `kv` feature and store key value pairs as attributes

    Log {
        level: convert_log_level_to_sentry_log_level(record.level()),
        body: format!("{}", record.args()),
        trace_id: None,
        timestamp: SystemTime::now(),
        severity_number: None,
        attributes,
    }
}
