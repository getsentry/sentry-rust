//! Adds support for automatic breadcrumb capturing from logs
//! by implementing the `slog::Drain`
//!
//! **Feature:** `with_slog`
use slog::{Drain, Level as SlogLevel, Never, OwnedKVList, Record};

use crate::api::add_breadcrumb;
use crate::backtrace_support::current_stacktrace;
use crate::hub::Hub;
use crate::protocol::{Breadcrumb, Event, Exception, Level};

/// Logger specific options.
pub struct LoggerOptions {
    /// The global filter that should be used (also used before dispatching
    /// to the nested logger).
    pub global_filter: Option<log::LevelFilter>,
    /// The sentry specific log level filter (defaults to `Info`)
    pub filter: log::LevelFilter,
    /// If set to `true`, breadcrumbs will be emitted. (defaults to `true`)
    pub emit_breadcrumbs: bool,
    /// If set to `true` error events will be sent for errors in the log. (defaults to `true`)
    pub emit_error_events: bool,
    /// If set to `true` warning events will be sent for warnings in the log. (defaults to `false`)
    pub emit_warning_events: bool,
    /// If set to `true` current stacktrace will be resolved and attached
    /// to each event. (expensive, defaults to `true`)
    pub attach_stacktraces: bool,
}

impl Default for LoggerOptions {
    fn default() -> LoggerOptions {
        LoggerOptions {
            global_filter: None,
            filter: log::LevelFilter::Info,
            emit_breadcrumbs: true,
            emit_error_events: true,
            emit_warning_events: false,
            attach_stacktraces: true,
        }
    }
}

impl LoggerOptions {
    /// Checks if an issue should be created.
    fn create_issue_for_record(&self, record: &log::Record<'_>) -> bool {
        match record.level() {
            log::Level::Warn => self.emit_warning_events,
            log::Level::Error => self.emit_error_events,
            _ => false,
        }
    }
}

/// Provides a logger that wraps the sentry communication.
pub struct Logger {
    options: LoggerOptions,
}

impl Logger {
    /// Initializes a new logger.
    pub fn new(options: LoggerOptions) -> Logger {
        Logger { options }
    }

    /// Returns the options of the logger.
    pub fn options(&self) -> &LoggerOptions {
        &self.options
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
    let culprit = format!(
        "{}:{}",
        record.file().unwrap_or("<unknown>"),
        record.line().unwrap_or(0)
    );
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
        culprit: Some(culprit),
        ..Default::default()
    }
}

fn convert_log_level(level: log::Level) -> Level {
    match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warning,
        log::Level::Info => Level::Info,
        log::Level::Debug | log::Level::Trace => Level::Debug,
    }
}

impl Drain for Logger {
    type Ok = ();
    type Err = Never;

    fn log(&self, record: &Record, _values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        let level = to_log_level(record.level());
        let md = log::MetadataBuilder::new()
            .level(level)
            .target(record.tag())
            .build();
        let args = *record.msg();

        let record = log::RecordBuilder::new()
            .metadata(md)
            .args(args)
            .module_path(Some(record.module()))
            .line(Some(record.line()))
            .file(Some(record.file()))
            .build();

        if self.options.create_issue_for_record(&record) {
            Hub::with_active(|hub| {
                hub.capture_event(event_from_record(&record, self.options.attach_stacktraces))
            });
        }
        if self.options.emit_breadcrumbs && record.level() <= self.options.filter {
            add_breadcrumb(|| breadcrumb_from_record(&record))
        }

        Ok(())
    }

    fn is_enabled(&self, level: SlogLevel) -> bool {
        let level = to_log_level(level);
        if let Some(global_filter) = self.options.global_filter {
            if level > global_filter {
                return false;
            }
        }
        level <= self.options.filter
    }
}

fn to_log_level(level: SlogLevel) -> log::Level {
    match level {
        SlogLevel::Trace => log::Level::Trace,
        SlogLevel::Debug => log::Level::Debug,
        SlogLevel::Info => log::Level::Info,
        SlogLevel::Warning => log::Level::Warn,
        SlogLevel::Error | SlogLevel::Critical => log::Level::Error,
    }
}
