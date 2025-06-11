use log::Record;
use sentry_core::protocol::{Breadcrumb, Event};
use sentry_core::sentry_debug;

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
//#[derive(Debug)]
pub struct SentryLogger<L: log::Log> {
    dest: L,
    filter: Box<dyn Fn(&log::Metadata<'_>) -> LogFilter + Send + Sync>,
    #[allow(clippy::type_complexity)]
    mapper: Option<Box<dyn Fn(&Record<'_>) -> RecordMapping + Send + Sync>>,
}

impl Default for SentryLogger<NoopLogger> {
    fn default() -> Self {
        sentry_debug!("[SentryLogger] Creating default SentryLogger with NoopLogger");
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
        sentry_debug!("[SentryLogger] Creating new SentryLogger");
        Default::default()
    }
}

impl<L: log::Log> SentryLogger<L> {
    /// Create a new SentryLogger wrapping a destination [`log::Log`].
    pub fn with_dest(dest: L) -> Self {
        sentry_debug!("[SentryLogger] Creating SentryLogger with custom destination");
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
        sentry_debug!("[SentryLogger] Setting custom filter function");
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
        sentry_debug!("[SentryLogger] Setting custom mapper function");
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
            Some(mapper) => {
                sentry_debug!("[SentryLogger] Using custom mapper for log record at {}", record.level());
                mapper(record)
            },
            None => {
                let filter_result = (self.filter)(record.metadata());
                sentry_debug!("[SentryLogger] Filter result for {} log: {:?}", record.level(), filter_result);
                match filter_result {
                    LogFilter::Ignore => RecordMapping::Ignore,
                    LogFilter::Breadcrumb => RecordMapping::Breadcrumb(breadcrumb_from_record(record)),
                    LogFilter::Event => RecordMapping::Event(event_from_record(record)),
                    LogFilter::Exception => RecordMapping::Event(exception_from_record(record)),
                }
            },
        };

        match item {
            RecordMapping::Ignore => {
                sentry_debug!("[SentryLogger] Ignoring {} log record", record.level());
            }
            RecordMapping::Breadcrumb(b) => {
                sentry_debug!("[SentryLogger] Adding breadcrumb from {} log", record.level());
                sentry_core::add_breadcrumb(b);
            }
            RecordMapping::Event(e) => {
                sentry_debug!("[SentryLogger] Capturing event from {} log: {}", record.level(), e.event_id);
                sentry_core::capture_event(e);
            }
        }

        self.dest.log(record)
    }

    fn flush(&self) {
        sentry_debug!("[SentryLogger] Flushing logger");
        self.dest.flush()
    }
}
