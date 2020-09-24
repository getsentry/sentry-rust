use sentry_core::protocol::{Event, Exception, Frame, Stacktrace};
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
        message: Some(format!("{}", record.args())),
        ..Default::default()
    }
}

/// Creates an [`Event`] from a given [`log::Record`].
pub fn event_from_record(record: &log::Record<'_>) -> Event<'static> {
    Event {
        logger: Some(record.target().into()),
        level: convert_log_level(record.level()),
        message: Some(format!("{}", record.args())),
        ..Default::default()
    }
}

/// Creates an exception [`Event`] from a given [`log::Record`].
pub fn exception_from_record(record: &log::Record<'_>) -> Event<'static> {
    let mut event = event_from_record(record);
    let frame = Frame {
        module: record.module_path().map(ToOwned::to_owned),
        filename: record.file().map(ToOwned::to_owned),
        lineno: record.line().map(Into::into),
        ..Default::default()
    };
    let exception = Exception {
        ty: record.target().into(),
        value: event.message.clone(),
        stacktrace: Some(Stacktrace {
            frames: vec![frame],
            ..Default::default()
        }),
        ..Default::default()
    };
    event.exception = vec![exception].into();
    event
}
