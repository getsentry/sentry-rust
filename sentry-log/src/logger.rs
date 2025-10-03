use log::Record;
use sentry_core::protocol::{Breadcrumb, Event};

use bitflags::bitflags;

#[cfg(feature = "logs")]
use crate::converters::log_from_record;
use crate::converters::{breadcrumb_from_record, event_from_record, exception_from_record};

bitflags! {
    /// The action that Sentry should perform for a [`log::Metadata`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LogFilter: u32 {
        /// Ignore the [`Record`].
        const Ignore = 0b0000;
        /// Create a [`Breadcrumb`] from this [`Record`].
        const Breadcrumb = 0b0001;
        /// Create a message [`Event`] from this [`Record`].
        const Event = 0b0010;
        /// Create an exception [`Event`] from this [`Record`].
        const Exception = 0b0100;
        /// Create a [`sentry_core::protocol::Log`] from this [`Record`].
        #[cfg(feature = "logs")]
        const Log = 0b1000;
    }
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
    /// Captures the [`sentry_core::protocol::Log`] to Sentry.
    #[cfg(feature = "logs")]
    Log(sentry_core::protocol::Log),
    /// Captures multiple items to Sentry.
    /// Nesting multiple `RecordMapping::Combined` is not supported and will cause the mappings to
    /// be ignored.
    Combined(Vec<RecordMapping>),
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
        self.dest.enabled(metadata) || !((self.filter)(metadata) == LogFilter::Ignore)
    }

    fn log(&self, record: &log::Record<'_>) {
        let items = match &self.mapper {
            Some(mapper) => mapper(record),
            None => {
                let filter = (self.filter)(record.metadata());
                let mut items = vec![];
                if filter.contains(LogFilter::Breadcrumb) {
                    items.push(RecordMapping::Breadcrumb(breadcrumb_from_record(record)));
                }
                if filter.contains(LogFilter::Event) {
                    items.push(RecordMapping::Event(event_from_record(record)));
                }
                if filter.contains(LogFilter::Exception) {
                    items.push(RecordMapping::Event(exception_from_record(record)));
                }
                #[cfg(feature = "logs")]
                if filter.contains(LogFilter::Log) {
                    items.push(RecordMapping::Log(log_from_record(record)));
                }
                RecordMapping::Combined(items)
            }
        };

        fn handle_single_mapping(mapping: RecordMapping) {
            match mapping {
                RecordMapping::Ignore => {}
                RecordMapping::Breadcrumb(breadcrumb) => sentry_core::add_breadcrumb(breadcrumb),
                RecordMapping::Event(event) => {
                    sentry_core::capture_event(event);
                }
                #[cfg(feature = "logs")]
                RecordMapping::Log(log) => {
                    sentry_core::Hub::with_active(|hub| hub.capture_log(log))
                }
                RecordMapping::Combined(_) => {
                    sentry_core::sentry_debug!(
                        "[SentryLogger] found nested RecordMapping::Combined, ignoring"
                    )
                }
            }
        }

        if let RecordMapping::Combined(items) = items {
            for item in items {
                handle_single_mapping(item);
            }
        } else {
            handle_single_mapping(items);
        }

        self.dest.log(record)
    }

    fn flush(&self) {
        self.dest.flush()
    }
}
