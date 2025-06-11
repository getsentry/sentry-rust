use log::Record;
use sentry_core::protocol::{Breadcrumb, Event};

use crate::converters::{breadcrumb_from_record, event_from_record, exception_from_record};

/// The action that Sentry should perform for a [`log::Metadata`].
#[derive(Debug)]
pub enum LogFilter {
    /// Ignore the [`Record`].
    Ignore,
    /// Create a [`Breadcrumb`] from this [`Record`].
    Breadcrumb,
    /// Create a message [`Event`] from this [`Record`].
    Event,
    /// Create an exception [`Event`] from this [`Record`].
    Exception,
}

/// The type of Data Sentry should ingest for a [`log::Record`].
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum RecordMapping {
    /// Ignore the [`Record`].
    Ignore,
    /// Adds the [`Breadcrumb`] to the Sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the [`Event`] to Sentry.
    Event(Event<'static>),
}

/// The default log filter.
///
/// By default, an exception event is captured for `error`, a breadcrumb for
/// `warning` and `info`, and `debug` and `trace` logs are ignored.
pub fn default_filter(metadata: &log::Metadata) -> LogFilter {
    match metadata.level() {
        log::Level::Error => LogFilter::Exception,
        log::Level::Warn | log::Level::Info => LogFilter::Breadcrumb,
        log::Level::Debug | log::Level::Trace => LogFilter::Ignore,
    }
}

/// A noop [`log::Log`] that just ignores everything.
#[derive(Debug, Default)]
pub struct NoopLogger;

impl log::Log for NoopLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let _ = metadata;
        false
    }

    fn log(&self, record: &log::Record) {
        let _ = record;
    }

    fn flush(&self) {
        todo!()
    }
}

/// Provides a dispatching logger.
///
/// A logger implementation that forwards logs to both a destination logger and Sentry.
/// This allows you to keep your existing logging setup while automatically capturing
/// important logs as Sentry events or breadcrumbs.
///
/// The `SentryLogger` acts as a wrapper around any existing [`log::Log`] implementation,
/// forwarding all log records to it while also processing them for Sentry based on
/// configurable filters.
///
/// By default:
/// - `ERROR` level logs become Sentry exception events
/// - `WARN` and `INFO` level logs become Sentry breadcrumbs
/// - `DEBUG` and `TRACE` level logs are ignored by Sentry
///
/// # Examples
///
/// ## Basic usage with existing logger
/// ```
/// use sentry_log::{SentryLogger, LogFilter};
/// 
/// // Wrap your existing logger
/// let logger = SentryLogger::with_dest(env_logger::Builder::new().build());
/// log::set_boxed_logger(Box::new(logger)).unwrap();
/// log::set_max_level(log::LevelFilter::Info);
/// 
/// // This will appear in both your regular logs and as a Sentry breadcrumb
/// log::info!("User logged in");
/// 
/// // This will appear in both your regular logs and as a Sentry error event
/// log::error!("Database connection failed");
/// ```
///
/// ## Custom filtering
/// ```
/// use sentry_log::{SentryLogger, LogFilter};
/// 
/// let logger = SentryLogger::new()
///     .filter(|metadata| match metadata.level() {
///         log::Level::Error => LogFilter::Event,
///         log::Level::Warn => LogFilter::Breadcrumb,
///         _ => LogFilter::Ignore, // Only capture errors and warnings
///     });
/// ```
///
/// ## Custom mapping for more control
/// ```
/// use sentry_log::{SentryLogger, RecordMapping};
/// use sentry_core::protocol::{Breadcrumb, Level};
/// 
/// let logger = SentryLogger::new()
///     .mapper(|record| {
///         if record.target().starts_with("my_app") {
///             // Only process logs from our application
///             RecordMapping::Breadcrumb(Breadcrumb {
///                 message: Some(record.args().to_string()),
///                 level: match record.level() {
///                     log::Level::Error => Level::Error,
///                     log::Level::Warn => Level::Warning,
///                     _ => Level::Info,
///                 },
///                 ..Default::default()
///             })
///         } else {
///             RecordMapping::Ignore
///         }
///     });
/// ```
//#[derive(Debug)]
pub struct SentryLogger<L: log::Log> {
    dest: L,
    filter: Box<dyn Fn(&log::Metadata<'_>) -> LogFilter + Send + Sync>,
    #[allow(clippy::type_complexity)]
    mapper: Option<Box<dyn Fn(&Record<'_>) -> RecordMapping + Send + Sync>>,
}

impl Default for SentryLogger<NoopLogger> {
    fn default() -> Self {
        Self {
            dest: NoopLogger,
            filter: Box::new(default_filter),
            mapper: None,
        }
    }
}

impl SentryLogger<NoopLogger> {
    /// Create a new SentryLogger with a [`NoopLogger`] as destination.
    pub fn new() -> Self {
        Default::default()
    }
}

impl<L: log::Log> SentryLogger<L> {
    /// Create a new SentryLogger wrapping a destination [`log::Log`].
    pub fn with_dest(dest: L) -> Self {
        Self {
            dest,
            filter: Box::new(default_filter),
            mapper: None,
        }
    }

    /// Sets a custom filter function.
    ///
    /// The filter classifies how sentry should handle [`Record`]s based on
    /// their [`log::Metadata`].
    #[must_use]
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&log::Metadata<'_>) -> LogFilter + Send + Sync + 'static,
    {
        self.filter = Box::new(filter);
        self
    }

    /// Sets a custom mapper function.
    ///
    /// The mapper is responsible for creating either breadcrumbs or events
    /// from [`Record`]s.
    #[must_use]
    pub fn mapper<M>(mut self, mapper: M) -> Self
    where
        M: Fn(&Record<'_>) -> RecordMapping + Send + Sync + 'static,
    {
        self.mapper = Some(Box::new(mapper));
        self
    }
}

impl<L: log::Log> log::Log for SentryLogger<L> {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.dest.enabled(metadata) || !matches!((self.filter)(metadata), LogFilter::Ignore)
    }

    fn log(&self, record: &log::Record<'_>) {
        let item: RecordMapping = match &self.mapper {
            Some(mapper) => mapper(record),
            None => match (self.filter)(record.metadata()) {
                LogFilter::Ignore => RecordMapping::Ignore,
                LogFilter::Breadcrumb => RecordMapping::Breadcrumb(breadcrumb_from_record(record)),
                LogFilter::Event => RecordMapping::Event(event_from_record(record)),
                LogFilter::Exception => RecordMapping::Event(exception_from_record(record)),
            },
        };

        match item {
            RecordMapping::Ignore => {}
            RecordMapping::Breadcrumb(b) => sentry_core::add_breadcrumb(b),
            RecordMapping::Event(e) => {
                sentry_core::capture_event(e);
            }
        }

        self.dest.log(record)
    }

    fn flush(&self) {
        self.dest.flush()
    }
}
