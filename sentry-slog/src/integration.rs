use sentry_core::protocol::{Breadcrumb, Event};
use sentry_core::{Hub, Integration};
use slog::{OwnedKVList, Record};

use crate::{breadcrumb_from_record, event_from_record, exception_from_record};

/// The Action that Sentry should perform for a `slog::Level`.
pub enum LevelFilter {
    /// Ignore the `Record`.
    Ignore,
    /// Create a `Breadcrumb` from this `Record`.
    Breadcrumb,
    /// Create a message `Event` from this `Record`.
    Event,
    /// Create an exception `Event` from this `Record`.
    Exception,
}

/// Custom Mappers
#[allow(clippy::large_enum_variant)]
pub enum RecordMapping {
    /// Adds the `Breadcrumb` to the sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the `Event` to sentry.
    Event(Event<'static>),
}

/// The default slog filter.
///
/// By default, an exception event is captured for `critical` logs,
/// a regular event for `error` and `warning` logs, and breadcrumbs for
/// everything else.
pub fn default_filter(level: slog::Level) -> LevelFilter {
    match level {
        slog::Level::Critical => LevelFilter::Exception,
        slog::Level::Error | slog::Level::Warning => LevelFilter::Event,
        slog::Level::Info | slog::Level::Debug | slog::Level::Trace => LevelFilter::Breadcrumb,
    }
}

/// The Sentry `slog` Integration.
///
/// Can be configured with a custom filter and mapper.
pub struct SlogIntegration {
    filter: Box<dyn Fn(slog::Level) -> LevelFilter + Send + Sync>,
    mapper: Option<Box<dyn Fn(&Record, &OwnedKVList) -> RecordMapping + Send + Sync>>,
}

impl Default for SlogIntegration {
    fn default() -> Self {
        Self {
            filter: Box::new(default_filter),
            mapper: None,
        }
    }
}

impl SlogIntegration {
    /// Create a new `slog` Integration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a custom filter function.
    ///
    /// The filter classifies how sentry should handle `slog::Record`s based on
    /// their level.
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
    /// from `slog::Record`s.
    pub fn mapper<M>(mut self, mapper: M) -> Self
    where
        M: Fn(&Record, &OwnedKVList) -> RecordMapping + Send + Sync + 'static,
    {
        self.mapper = Some(Box::new(mapper));
        self
    }

    pub(crate) fn log(&self, hub: &Hub, record: &Record, values: &OwnedKVList) {
        let item: RecordMapping = match &self.mapper {
            Some(mapper) => mapper(record, values),
            None => match (self.filter)(record.level()) {
                LevelFilter::Ignore => return,
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
            RecordMapping::Breadcrumb(b) => hub.add_breadcrumb(b),
            RecordMapping::Event(e) => {
                hub.capture_event(e);
            }
        }
    }

    pub(crate) fn is_enabled(&self, level: slog::Level) -> bool {
        match (self.filter)(level) {
            LevelFilter::Ignore => false,
            _ => true,
        }
    }
}

impl Integration for SlogIntegration {
    fn name(&self) -> &'static str {
        "slog"
    }
}
