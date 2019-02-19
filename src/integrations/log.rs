//! Adds support for automatic breadcrumb capturing from logs.
//!
//! **Feature:** `with_log` (*enabled by default*)
//!
//! The `log` crate is supported in two ways.  First events can be captured as
//! breadcrumbs for later, secondly error events can be logged as events to
//! Sentry.  By default anything above `Info` is recorded as breadcrumb and
//! anything above `Error` is captured as error event.
//!
//! # Configuration
//!
//! However due to how log systems in Rust work this currently requires you to
//! slightly change your log setup.  This is an example with the pretty
//! env logger crate:
//!
//! ```no_run
//! # extern crate sentry;
//! # extern crate pretty_env_logger;
//! let mut log_builder = pretty_env_logger::formatted_builder();
//! log_builder.parse("info");  // or env::var("RUST_LOG")
//! let logger = log_builder.build();
//! let options = sentry::integrations::log::LoggerOptions {
//!     global_filter: Some(logger.filter()),
//!     ..Default::default()
//! };
//! sentry::integrations::log::init(Some(Box::new(logger)), options);
//! ```
//!
//! For loggers based on `env_logger` (like `pretty_env_logger`) you can also
//! use the [`env_logger`](../env_logger/index.html) integration which is
//! much easier to use.
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
    /// Returns the effective global filter.
    ///
    /// This is what is set for these logger options when the log level
    /// needs to be set globally.  This is the greater of `global_filter`
    /// and `filter`.
    #[inline(always)]
    fn effective_global_filter(&self) -> log::LevelFilter {
        let filter = if let Some(filter) = self.global_filter {
            if filter < self.filter {
                self.filter
            } else {
                filter
            }
        } else {
            self.filter
        };
        std::cmp::max(filter, self.issue_filter())
    }

    /// Returns the level for which issues should be created.
    ///
    /// This is controlled by `emit_error_events` and `emit_warning_events`.
    #[inline(always)]
    fn issue_filter(&self) -> log::LevelFilter {
        if self.emit_warning_events {
            log::LevelFilter::Warn
        } else if self.emit_error_events {
            log::LevelFilter::Error
        } else {
            log::LevelFilter::Off
        }
    }

    /// Checks if an issue should be created.
    fn create_issue_for_record(&self, record: &log::Record<'_>) -> bool {
        match record.level() {
            log::Level::Warn => self.emit_warning_events,
            log::Level::Error => self.emit_error_events,
            _ => false,
        }
    }
}

/// Provides a dispatching logger.
pub struct Logger {
    dest: Option<Box<dyn log::Log>>,
    options: LoggerOptions,
}

impl Logger {
    /// Initializes a new logger.
    ///
    /// It can just send to Sentry or additionally also send messages to another
    /// logger.
    pub fn new(dest: Option<Box<dyn log::Log>>, options: LoggerOptions) -> Logger {
        Logger { dest, options }
    }

    /// Returns the options of the logger.
    pub fn options(&self) -> &LoggerOptions {
        &self.options
    }

    /// Returns the destination logger.
    pub fn dest_log(&self) -> Option<&dyn log::Log> {
        self.dest.as_ref().map(|x| &**x)
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

impl log::Log for Logger {
    fn enabled(&self, md: &log::Metadata<'_>) -> bool {
        if let Some(global_filter) = self.options.global_filter {
            if md.level() > global_filter {
                return false;
            }
        }
        md.level() <= self.options.filter || self.dest.as_ref().map_or(false, |x| x.enabled(md))
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.options.create_issue_for_record(record) {
            Hub::with_active(|hub| hub.capture_event(
                event_from_record(record, self.options.attach_stacktraces)));
        }
        if self.options.emit_breadcrumbs && record.level() <= self.options.filter {
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
/// let logger = builder.build();
/// log::init(Some(Box::new(builder.build())), LoggerOptions {
///     global_filter: Some(logger.filter()),
///     ..Default::default()
/// });
/// ```
///
/// (For using `env_logger` you can also use the `env_logger` integration
/// which simplifies this).
pub fn init(dest: Option<Box<dyn log::Log>>, options: LoggerOptions) {
    let logger = Logger::new(dest, options);
    let filter = logger.options().effective_global_filter();
    if filter > log::max_level() {
        log::set_max_level(filter);
    }
    log::set_boxed_logger(Box::new(logger)).unwrap();
}

#[test]
fn test_filters() {
    let opt_warn = LoggerOptions {
        filter: log::LevelFilter::Warn,
        ..Default::default()
    };
    assert_eq!(opt_warn.effective_global_filter(), log::LevelFilter::Warn);
    assert_eq!(opt_warn.issue_filter(), log::LevelFilter::Error);

    let opt_debug = LoggerOptions {
        global_filter: Some(log::LevelFilter::Debug),
        filter: log::LevelFilter::Warn,
        ..Default::default()
    };
    assert_eq!(opt_debug.effective_global_filter(), log::LevelFilter::Debug);

    let opt_debug_inverse = LoggerOptions {
        global_filter: Some(log::LevelFilter::Warn),
        filter: log::LevelFilter::Debug,
        ..Default::default()
    };
    assert_eq!(
        opt_debug_inverse.effective_global_filter(),
        log::LevelFilter::Debug
    );

    let opt_weird = LoggerOptions {
        filter: log::LevelFilter::Error,
        emit_warning_events: true,
        ..Default::default()
    };
    assert_eq!(opt_weird.issue_filter(), log::LevelFilter::Warn);
    assert_eq!(opt_weird.effective_global_filter(), log::LevelFilter::Warn);
}
