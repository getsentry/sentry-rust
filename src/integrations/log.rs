//! Adds support for automatic breadcrumb capturing from logs.
use log;

use protocol::{Breadcrumb, Event, Exception, Level};
use api::add_breadcrumb;
use utils::current_stacktrace;
use scope::with_client_and_scope;

/// Logger specific options.
pub struct LoggerOptions {
    /// The global filter that should be used (also used before dispatching
    /// to the nested logger).
    pub global_filter: Option<log::LevelFilter>,
    /// The sentry specific log level filter (defaults to `Info`)
    pub filter: log::LevelFilter,
    /// If set to `true`, breadcrumbs are emitted. (defaults to `true`)
    pub emit_breadcrumbs: bool,
    /// If set to `true` error events are sent for errors in the log. (defaults to `true`)
    pub emit_error_events: bool,
}

impl Default for LoggerOptions {
    fn default() -> LoggerOptions {
        LoggerOptions {
            global_filter: None,
            filter: log::LevelFilter::Info,
            emit_breadcrumbs: true,
            emit_error_events: true,
        }
    }
}

/// Provides a dispatching logger.
pub struct Logger {
    dest: Option<Box<log::Log>>,
    options: LoggerOptions,
}

impl Logger {
    /// Initializes a new logger.
    ///
    /// It can just send to Sentry or additionally also send messages to another
    /// logger.
    pub fn new(dest: Option<Box<log::Log>>, options: LoggerOptions) -> Logger {
        Logger { dest, options }
    }

    /// Returns the options of the logger.
    pub fn options(&self) -> &LoggerOptions {
        &self.options
    }

    /// Returns the destination logger.
    pub fn dest_log(&self) -> Option<&log::Log> {
        self.dest.as_ref().map(|x| &**x)
    }
}

/// Creates a breadcrumb from a given log record.
pub fn breadcrumb_from_record(record: &log::Record) -> Breadcrumb {
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
pub fn event_from_record(record: &log::Record, with_stacktrace: bool) -> Event {
    Event {
        logger: Some(record.target().into()),
        level: convert_log_level(record.level()),
        exceptions: vec![
            Exception {
                ty: record.target().into(),
                value: Some(format!("{}", record.args())),
                stacktrace: if with_stacktrace {
                    current_stacktrace()
                } else {
                    None
                },
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

impl log::Log for Logger {
    fn enabled(&self, md: &log::Metadata) -> bool {
        if let Some(global_filter) = self.options.global_filter {
            if md.level() < global_filter {
                return false;
            }
        }
        md.level() <= self.options.filter || self.dest.as_ref().map_or(false, |x| x.enabled(md))
    }

    fn log(&self, record: &log::Record) {
        if self.options.emit_error_events && record.level() <= log::Level::Error {
            with_client_and_scope(|client, scope| {
                client.capture_event(event_from_record(record, true), Some(scope))
            });
        }
        if record.level() <= self.options.filter {
            add_breadcrumb(|| breadcrumb_from_record(record))
        }
        if let Some(ref log) = self.dest {
            if log.enabled(record.metadata()) {
                log.log(record);
            }
        }
    }

    fn flush(&self) {
        if let Some(ref log) = self.dest {
            log.flush();
        }
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

/// Initializes the logging system.
///
/// This takes a destination logger to which Sentry should forward all
/// intercepted log messages and the options for the log handler.
///
/// Typically a log system in Rust will call `log::set_logger` itself
/// but since we need to intercept this, a user of this function will
/// need to pass a logger to it instead of calling the init function of
/// the other crate.
///
/// For instance to use `env_logger` with this one needs to do this:
///
/// ```ignore
/// use sentry::integrations::log;
/// use env_logger;
///
/// let builder = env_logger::Builder::from_default_env();
/// log::init(Some(Box::new(builder.build())), Default::default());
/// ```
pub fn init(dest: Option<Box<log::Log>>, options: LoggerOptions) {
    let logger = Logger::new(dest, options);
    if let Some(filter) = logger.options().global_filter {
        log::set_max_level(filter);
    } else {
        log::set_max_level(logger.options().filter);
    }
    log::set_boxed_logger(Box::new(logger)).unwrap();
}
