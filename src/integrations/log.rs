//! Adds support for automatic breadcrumb capturing from logs.
use log;

use protocol::Breadcrumb;
use api::add_breadcrumb_from;


/// Logger specific options.
pub struct LoggerOptions {
    /// The sentry specific log level filter
    pub filter: log::LevelFilter,
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
}

impl log::Log for Logger {
    fn enabled(&self, md: &log::Metadata) -> bool {
        self.options.filter >= md.level() || self.dest.as_ref().map_or(false, |x| x.enabled(md))
    }

    fn log(&self, record: &log::Record) {
        if self.options.filter >= record.level() {
            add_breadcrumb_from(move || Breadcrumb {
                message: Some(format!("{}", record.args())),
                ..Default::default()
            });
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
