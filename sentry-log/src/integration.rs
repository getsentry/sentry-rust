#![allow(deprecated)]

use std::sync::Once;

use log::Record;
use sentry_core::protocol::{Breadcrumb, Event};
use sentry_core::{ClientOptions, Integration};

use crate::logger::Logger;

/// The Action that Sentry should perform for a [`log::Metadata`].
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

/// The type of Data Sentry should ingest for a [`log::Record`].
#[allow(clippy::large_enum_variant)]
pub enum RecordMapping {
    /// Ignore the [`Record`]
    Ignore,
    /// Adds the [`Breadcrumb`] to the Sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the [`Event`] to Sentry.
    Event(Event<'static>),
}

/// Logger specific options.
pub struct LogIntegration {
    /// The global filter that should be used (also used before dispatching
    /// to the nested logger).
    #[deprecated = "use the [`filter()`] function instead"]
    pub global_filter: Option<log::LevelFilter>,
    /// The sentry specific log level filter (defaults to `Info`)
    #[deprecated = "use the [`filter()`] function instead"]
    pub filter: log::LevelFilter,
    /// If set to `true`, breadcrumbs will be emitted. (defaults to `true`)
    #[deprecated = "use the [`filter()`] function instead"]
    pub emit_breadcrumbs: bool,
    /// If set to `true` error events will be sent for errors in the log. (defaults to `true`)
    #[deprecated = "use the [`filter()`] function instead"]
    pub emit_error_events: bool,
    /// If set to `true` warning events will be sent for warnings in the log. (defaults to `false`)
    #[deprecated = "use the [`filter()`] function instead"]
    pub emit_warning_events: bool,
    /// If set to `true` current stacktrace will be resolved and attached
    /// to each event. (expensive, defaults to `true`)
    #[deprecated = "use builder functions instead; direct field access will be removed soon"]
    pub attach_stacktraces: bool,
    /// The destination log.
    #[deprecated = "use builder functions instead; direct field access will be removed soon"]
    pub dest_log: Option<Box<dyn log::Log>>,

    sentry_filter: Option<Box<dyn Fn(&log::Metadata<'_>) -> LevelFilter + Send + Sync>>,
    mapper: Option<Box<dyn Fn(&Record<'_>) -> RecordMapping + Send + Sync>>,
}

static INIT: Once = Once::new();

impl Integration for LogIntegration {
    fn name(&self) -> &'static str {
        "log"
    }

    fn setup(&self, cfg: &mut ClientOptions) {
        cfg.in_app_exclude.push("log::");
        cfg.extra_border_frames
            .push("<sentry_log::Logger as log::Log>::log");

        let filter = self.effective_global_filter();
        if filter > log::max_level() {
            log::set_max_level(filter);
        }

        INIT.call_once(|| {
            log::set_boxed_logger(Box::new(Logger::default())).ok();
        });
    }
}

impl Default for LogIntegration {
    fn default() -> Self {
        Self {
            global_filter: None,
            filter: log::LevelFilter::Info,
            emit_breadcrumbs: true,
            emit_error_events: true,
            emit_warning_events: false,
            attach_stacktraces: true,
            dest_log: None,
            sentry_filter: None,
            mapper: None,
        }
    }
}

impl std::fmt::Debug for LogIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        struct DestLog;
        let dest_log = self.dest_log.as_ref().map(|_| DestLog);

        f.debug_struct("LogIntegration")
            .field("global_filter", &self.global_filter)
            .field("filter", &self.filter)
            .field("emit_breadcrumbs", &self.emit_breadcrumbs)
            .field("emit_error_events", &self.emit_error_events)
            .field("emit_warning_events", &self.emit_warning_events)
            .field("attach_stacktraces", &self.attach_stacktraces)
            .field("dest_log", &dest_log)
            .finish()
    }
}

impl LogIntegration {
    /// Creates a new `log` Integration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initializes an env logger as destination target.
    #[cfg(feature = "env_logger")]
    pub fn with_env_logger_dest(mut self, logger: Option<env_logger::Logger>) -> Self {
        let logger = logger
            .unwrap_or_else(|| env_logger::Builder::from_env(env_logger::Env::default()).build());
        let filter = logger.filter();
        if self.global_filter.is_none() {
            self.global_filter = Some(filter);
        }
        self.dest_log = Some(Box::new(logger));
        self
    }

    /// Sets a custom filter function.
    ///
    /// The filter classifies how sentry should handle [`Record`]s based on
    /// their [`log::Metadata`].
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&log::Metadata<'_>) -> LevelFilter + Send + Sync + 'static,
    {
        self.sentry_filter = Some(Box::new(filter));
        self
    }

    /// Sets a custom mapper function.
    ///
    /// The mapper is responsible for creating either breadcrumbs or events
    /// from [`Record`]s.
    pub fn mapper<M>(mut self, mapper: M) -> Self
    where
        M: Fn(&Record<'_>) -> RecordMapping + Send + Sync + 'static,
    {
        self.mapper = Some(Box::new(mapper));
        self
    }

    /// Returns the effective global filter.
    ///
    /// This is what is set for these logger options when the log level
    /// needs to be set globally.  This is the greater of `global_filter`
    /// and `filter`.
    #[inline(always)]
    pub(crate) fn effective_global_filter(&self) -> log::LevelFilter {
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
    pub(crate) fn create_issue_for_record(&self, record: &Record<'_>) -> bool {
        match record.level() {
            log::Level::Warn => self.emit_warning_events,
            log::Level::Error => self.emit_error_events,
            _ => false,
        }
    }
}

#[test]
fn test_filters() {
    use log::LevelFilter;

    let opt_warn = LogIntegration {
        filter: LevelFilter::Warn,
        ..Default::default()
    };
    assert_eq!(opt_warn.effective_global_filter(), LevelFilter::Warn);
    assert_eq!(opt_warn.issue_filter(), LevelFilter::Error);

    let opt_debug = LogIntegration {
        global_filter: Some(LevelFilter::Debug),
        filter: LevelFilter::Warn,
        ..Default::default()
    };
    assert_eq!(opt_debug.effective_global_filter(), LevelFilter::Debug);

    let opt_debug_inverse = LogIntegration {
        global_filter: Some(LevelFilter::Warn),
        filter: LevelFilter::Debug,
        ..Default::default()
    };
    assert_eq!(
        opt_debug_inverse.effective_global_filter(),
        LevelFilter::Debug
    );

    let opt_weird = LogIntegration {
        filter: LevelFilter::Error,
        emit_warning_events: true,
        ..Default::default()
    };
    assert_eq!(opt_weird.issue_filter(), LevelFilter::Warn);
    assert_eq!(opt_weird.effective_global_filter(), LevelFilter::Warn);
}
