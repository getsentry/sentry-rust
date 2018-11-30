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
//! let mut log_builder = pretty_env_logger::formatted_builder().unwrap();
//! log_builder.parse("info");  // or env::var("RUST_LOG")
//! let logger = log_builder.build();
//! sentry::init(sentry::ClientOptions::default()
//!     .add_integration(sentry::integrations::log::LogIntegration {
//!         global_filter: Some(logger.filter()),
//!         dest_log: Some(Box::new(logger)),
//!         ..Default::default()
//!     }));
//! ```
//!
//! Additionally if the `with_env_logger` feature is enabled an `with_env_logger_dest`
//! method is available on the integration to directly forward to an env logger in
//! a more convenient way.
use log;
use std::cmp;
use std::fmt;

#[cfg(feature = "with_env_logger")]
use env_logger;

use api::add_breadcrumb;
use api::protocol::{Breadcrumb, Event, Exception, Level};
use backtrace_support::current_stacktrace;
use hub::Hub;

use client::ClientOptions;
use integrations::Integration;

/// Logger specific options.
pub struct LogIntegration {
    /// The global filter that should be used (also used before dispatching
    /// to the nested logger).
    pub global_filter: Option<log::LevelFilter>,
    /// The sentry specific log level filter (defaults to `Info`)
    pub filter: log::LevelFilter,
    /// If set to `true`, breadcrumbs are emitted. (defaults to `true`)
    pub emit_breadcrumbs: bool,
    /// If set to `true` error events are sent for errors in the log. (defaults to `true`)
    pub emit_error_events: bool,
    /// If set to `true` warning events are sent for warnings in the log. (defaults to `false`)
    pub emit_warning_events: bool,
    /// The destination log.
    pub dest_log: Option<Box<log::Log>>,
}

impl fmt::Debug for LogIntegration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LogIntegration")
            .field("global_filter", &self.global_filter)
            .field("filter", &self.filter)
            .field("emit_breadcrumbs", &self.emit_breadcrumbs)
            .field("emit_error_events", &self.emit_error_events)
            .field("emit_warning_events", &self.emit_warning_events)
            .finish()
    }
}

impl Default for LogIntegration {
    fn default() -> LogIntegration {
        LogIntegration {
            global_filter: None,
            filter: log::LevelFilter::Info,
            emit_breadcrumbs: true,
            emit_error_events: true,
            emit_warning_events: false,
            dest_log: None,
        }
    }
}

impl LogIntegration {
    /// Initializes an env logger as destination target.
    #[cfg(feature = "with_env_logger")]
    pub fn with_env_logger_dest(mut self, logger: Option<env_logger::Logger>) -> LogIntegration {
        let logger = logger
            .unwrap_or_else(|| env_logger::Builder::from_env(env_logger::Env::default()).build());
        let filter = logger.filter();
        if self.global_filter.is_none() {
            self.global_filter = Some(filter);
        }
        self.dest_log = Some(Box::new(logger));
        self
    }

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
        cmp::max(filter, self.issue_filter())
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
    fn create_issue_for_record(&self, record: &log::Record) -> bool {
        match record.level() {
            log::Level::Warn => self.emit_warning_events,
            log::Level::Error => self.emit_error_events,
            _ => false,
        }
    }
}

#[cfg(feature = "with_client_implementation")]
impl Integration for LogIntegration {
    fn setup(&self, _: &ClientOptions) {
        let filter = self.effective_global_filter();
        if filter > log::max_level() {
            log::set_max_level(filter);
        }
    }

    fn setup_once(&self) {
        let logger = Logger::new();
        log::set_boxed_logger(Box::new(logger)).unwrap();
    }
}

/// Provides a dispatching logger.
#[derive(Debug, Default)]
pub struct Logger;

impl Logger {
    /// Initializes a new logger.
    pub fn new() -> Logger {
        Logger::default()
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
pub fn event_from_record(record: &log::Record, with_stacktrace: bool) -> Event<'static> {
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
        }].into(),
        ..Default::default()
    }
}

impl log::Log for Logger {
    fn enabled(&self, md: &log::Metadata) -> bool {
        let integration = match Hub::with_active(|hub| hub.get_integration::<LogIntegration>()) {
            Some(value) => value,
            None => return false,
        };

        if let Some(global_filter) = integration.global_filter {
            if md.level() < global_filter {
                return false;
            }
        }
        md.level() <= integration.filter || integration
            .dest_log
            .as_ref()
            .map_or(false, |x| x.enabled(md))
    }

    fn log(&self, record: &log::Record) {
        Hub::with_active(|hub| {
            let integration = match hub.get_integration::<LogIntegration>() {
                Some(value) => value,
                None => return,
            };

            if integration.create_issue_for_record(record) {
                hub.capture_event(event_from_record(
                    record,
                    hub.client()
                        .map_or(false, |x| x.options().attach_stacktrace),
                ));
            }
            if record.level() <= integration.filter {
                add_breadcrumb(|| breadcrumb_from_record(record))
            }
            if let Some(ref log) = integration.dest_log {
                if log.enabled(record.metadata()) {
                    log.log(record);
                }
            }
        })
    }

    fn flush(&self) {
        let integration = match Hub::current().get_integration::<LogIntegration>() {
            Some(value) => value,
            None => return,
        };

        if let Some(ref log) = integration.dest_log {
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

#[test]
fn test_filters() {
    let opt_warn = LogIntegration {
        filter: log::LevelFilter::Warn,
        ..Default::default()
    };
    assert_eq!(opt_warn.effective_global_filter(), log::LevelFilter::Warn);
    assert_eq!(opt_warn.issue_filter(), log::LevelFilter::Error);

    let opt_debug = LogIntegration {
        global_filter: Some(log::LevelFilter::Debug),
        filter: log::LevelFilter::Warn,
        ..Default::default()
    };
    assert_eq!(opt_debug.effective_global_filter(), log::LevelFilter::Debug);

    let opt_debug_inverse = LogIntegration {
        global_filter: Some(log::LevelFilter::Warn),
        filter: log::LevelFilter::Debug,
        ..Default::default()
    };
    assert_eq!(
        opt_debug_inverse.effective_global_filter(),
        log::LevelFilter::Debug
    );

    let opt_weird = LogIntegration {
        filter: log::LevelFilter::Error,
        emit_warning_events: true,
        ..Default::default()
    };
    assert_eq!(opt_weird.issue_filter(), log::LevelFilter::Warn);
    assert_eq!(opt_weird.effective_global_filter(), log::LevelFilter::Warn);
}
