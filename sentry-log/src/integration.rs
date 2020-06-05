use std::sync::Once;

use log::{Level, LevelFilter, Record};
use sentry_core::{ClientOptions, Integration};

use crate::logger::Logger;

/// Logger specific options.
pub struct LogIntegration {
    /// The global filter that should be used (also used before dispatching
    /// to the nested logger).
    pub global_filter: Option<LevelFilter>,
    /// The sentry specific log level filter (defaults to `Info`)
    pub filter: LevelFilter,
    /// If set to `true`, breadcrumbs will be emitted. (defaults to `true`)
    pub emit_breadcrumbs: bool,
    /// If set to `true` error events will be sent for errors in the log. (defaults to `true`)
    pub emit_error_events: bool,
    /// If set to `true` warning events will be sent for warnings in the log. (defaults to `false`)
    pub emit_warning_events: bool,
    /// If set to `true` current stacktrace will be resolved and attached
    /// to each event. (expensive, defaults to `true`)
    pub attach_stacktraces: bool,
    /// The destination log.
    pub dest_log: Option<Box<dyn log::Log>>,
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

        INIT.call_once(|| log::set_boxed_logger(Box::new(Logger::default())).unwrap());
    }
}

impl Default for LogIntegration {
    fn default() -> Self {
        Self {
            global_filter: None,
            filter: LevelFilter::Info,
            emit_breadcrumbs: true,
            emit_error_events: true,
            emit_warning_events: false,
            attach_stacktraces: true,
            dest_log: None,
        }
    }
}

impl LogIntegration {
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

    /// Returns the effective global filter.
    ///
    /// This is what is set for these logger options when the log level
    /// needs to be set globally.  This is the greater of `global_filter`
    /// and `filter`.
    #[inline(always)]
    pub(crate) fn effective_global_filter(&self) -> LevelFilter {
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
    fn issue_filter(&self) -> LevelFilter {
        if self.emit_warning_events {
            LevelFilter::Warn
        } else if self.emit_error_events {
            LevelFilter::Error
        } else {
            LevelFilter::Off
        }
    }

    /// Checks if an issue should be created.
    pub(crate) fn create_issue_for_record(&self, record: &Record<'_>) -> bool {
        match record.level() {
            Level::Warn => self.emit_warning_events,
            Level::Error => self.emit_error_events,
            _ => false,
        }
    }
}

#[test]
fn test_filters() {
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
