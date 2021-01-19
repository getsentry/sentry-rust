use sentry_core::protocol::Event;
use sentry_core::{Breadcrumb, Level};

/// Converts a [`log::Level`] to a Sentry [`Level`]
pub fn convert_log_level(level: log::Level) -> Level {
    match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warning,
        log::Level::Info => Level::Info,
        log::Level::Debug | log::Level::Trace => Level::Debug,
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
