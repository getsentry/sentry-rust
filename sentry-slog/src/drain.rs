use sentry_core::protocol::{Breadcrumb, Event};
use slog::{Drain, OwnedKVList, Record};

use crate::{breadcrumb_from_record, event_from_record, exception_from_record};

/// The action that Sentry should perform for a [`slog::Level`].
#[derive(Debug)]
pub enum LevelFilter {
    /// Ignore the [`Record`].
    Ignore,
    /// Create a [`Breadcrumb`] from this [`Record`].
    Breadcrumb,
    /// Create a message [`Event`] from this [`Record`].
    Event,
    /// Create an exception [`Event`] from this [`Record`].
    Exception,
}

/// The type of Data Sentry should ingest for a [`slog::Record`].
#[allow(clippy::large_enum_variant)]
pub enum RecordMapping {
    /// Ignore the [`Record`].
    Ignore,
    /// Adds the [`Breadcrumb`] to the Sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the [`Event`] to Sentry.
    Event(Event<'static>),
}

/// The default slog filter.
///
/// By default, an exception event is captured for `critical` logs,
/// a regular event for `error`, a breadcrumb for `warning` and `info`, and
/// `debug` and `trace` logs are ignored.
pub fn default_filter(level: slog::Level) -> LevelFilter {
    match level {
        slog::Level::Critical => LevelFilter::Exception,
        slog::Level::Error | slog::Level::Warning => LevelFilter::Event,
        slog::Level::Info | slog::Level::Debug | slog::Level::Trace => LevelFilter::Breadcrumb,
    }
}

/// A Drain which passes all [`Record`]s to Sentry.
pub struct SentryDrain<D: Drain> {
    drain: D,
    filter: Box<dyn Fn(slog::Level) -> LevelFilter + Send + Sync>,
    mapper: Option<Box<dyn Fn(&Record, &OwnedKVList) -> RecordMapping + Send + Sync>>,
}

impl<D: slog::SendSyncRefUnwindSafeDrain> std::panic::RefUnwindSafe for SentryDrain<D> {}
impl<D: slog::SendSyncUnwindSafeDrain> std::panic::UnwindSafe for SentryDrain<D> {}

impl<D: Drain> SentryDrain<D> {
    /// Creates a new `SentryDrain`, wrapping a `slog::Drain`.
    pub fn new(drain: D) -> Self {
        Self {
            drain,
            filter: Box::new(default_filter),
            mapper: None,
        }
    }

    /// Sets a custom filter function.
    ///
    /// The filter classifies how sentry should handle [`Record`]s based on
    /// their [`slog::Level`].
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(slog::Level) -> LevelFilter + Send + Sync + 'static,
    {
        self.filter = Box::new(filter);
        self
    }

    /// Sets a custom mapper function.
    ///
    /// The mapper is responsible for creating either breadcrumbs or events
    /// from [`Record`]s.
    ///
    /// # Examples
    ///
    /// ```
    /// use sentry_slog::{breadcrumb_from_record, RecordMapping, SentryDrain};
    ///
    /// let drain = SentryDrain::new(slog::Discard).mapper(|record, kv| match record.level() {
    ///     slog::Level::Trace => RecordMapping::Ignore,
    ///     _ => RecordMapping::Breadcrumb(breadcrumb_from_record(record, kv)),
    /// });
    /// ```
    pub fn mapper<M>(mut self, mapper: M) -> Self
    where
        M: Fn(&Record, &OwnedKVList) -> RecordMapping + Send + Sync + 'static,
    {
        self.mapper = Some(Box::new(mapper));
        self
    }
}

impl<D: Drain> slog::Drain for SentryDrain<D> {
    type Ok = D::Ok;
    type Err = D::Err;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        let item: RecordMapping = match &self.mapper {
            Some(mapper) => mapper(record, values),
            None => match (self.filter)(record.level()) {
                LevelFilter::Ignore => RecordMapping::Ignore,
                LevelFilter::Breadcrumb => {
                    RecordMapping::Breadcrumb(breadcrumb_from_record(record, values))
                }
                LevelFilter::Event => RecordMapping::Event(event_from_record(record, values)),
                LevelFilter::Exception => {
                    RecordMapping::Event(exception_from_record(record, values))
                }
            },
        };

        match item {
            RecordMapping::Ignore => {}
            RecordMapping::Breadcrumb(b) => sentry_core::add_breadcrumb(b),
            RecordMapping::Event(e) => {
                sentry_core::capture_event(e);
            }
        }

        self.drain.log(record, values)
    }

    fn is_enabled(&self, level: slog::Level) -> bool {
        self.drain.is_enabled(level) || !matches!((self.filter)(level), LevelFilter::Ignore)
    }
}
