use sentry_backtrace::current_stacktrace;
use sentry_core::protocol::{Event, Exception};
use sentry_core::{Breadcrumb, Level};

fn convert_log_level(level: log::Level) -> Level {
    match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warning,
        log::Level::Info => Level::Info,
        log::Level::Debug | log::Level::Trace => Level::Debug,
    }
}

/// Creates a breadcrumb from a given log record.
pub fn breadcrumb_from_record(record: &log::Record<'_>) -> Breadcrumb {
    Breadcrumb {
        ty: "log".into(),
        level: convert_log_level(record.level()),
        category: Some(record.target().into()),
        message: Some(format!("{}", record.args())),
        ..Default::default()
    }
}

/// Creates an event from a given log record.
///
/// If `with_stacktrace` is set to `true` then a stacktrace is attached
/// from the current frame.
pub fn event_from_record(record: &log::Record<'_>, with_stacktrace: bool) -> Event<'static> {
    Event {
        logger: Some(record.target().into()),
        level: convert_log_level(record.level()),
        exception: vec![Exception {
            ty: record.target().into(),
            value: Some(format!("{}", record.args())),
            stacktrace: if with_stacktrace {
                current_stacktrace()
            } else {
                None
            },
            ..Default::default()
        }]
        .into(),
        ..Default::default()
    }
}
