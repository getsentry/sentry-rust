use std::sync::Once;

use log::LevelFilter;
use sentry_core::{ClientOptions, Integration};

use crate::filters::Filters;
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

        let filters = self.create_filters();
        let filter = filters.effective_global_filter();
        if filter > log::max_level() {
            log::set_max_level(filter);
        }

        INIT.call_once(move || {
            // NOTE on safety:
            // The way the current log-integration code is structured, we have
            // no way to move the `dest_logger` from the integration to our own
            // log instance without breaking the API.
            // The `Once` here makes sure that we only ever do this unsafe
            // `take` once, and there are no other pieces of code that read from
            // from this integration instance.
            let dest_log = unsafe {
                let const_ptr = &self.dest_log as *const Option<Box<dyn log::Log>>;
                let mut_ptr = const_ptr as *mut Option<Box<dyn log::Log>>;
                (&mut *mut_ptr).take()
            };
            let logger = Logger { filters, dest_log };
            log::set_boxed_logger(Box::new(logger)).ok();
        });
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

    pub(crate) fn create_filters(&self) -> Filters {
        Filters {
            global_filter: self.global_filter,
            filter: self.filter,
            emit_breadcrumbs: self.emit_breadcrumbs,
            emit_error_events: self.emit_error_events,
            emit_warning_events: self.emit_warning_events,
            attach_stacktraces: self.attach_stacktraces,
        }
    }
}
