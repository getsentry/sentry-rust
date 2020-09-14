use crate::converters::{breadcrumb_from_record, event_from_record};
use crate::filters::Filters;

/// Provides a dispatching logger.
#[derive(Default)]
pub struct Logger {
    pub(crate) filters: Filters,
    pub(crate) dest_log: Option<Box<dyn log::Log>>,
}

impl log::Log for Logger {
    fn enabled(&self, md: &log::Metadata<'_>) -> bool {
        if let Some(global_filter) = self.filters.global_filter {
            if md.level() > global_filter {
                return false;
            }
        }
        md.level() <= self.filters.filter || self.dest_log.as_ref().map_or(false, |x| x.enabled(md))
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.filters.create_issue_for_record(record) {
            sentry_core::capture_event(event_from_record(record, self.filters.attach_stacktraces));
        }
        if self.filters.emit_breadcrumbs && record.level() <= self.filters.filter {
            sentry_core::add_breadcrumb(|| breadcrumb_from_record(record));
        }
        if let Some(ref log) = self.dest_log {
            if log.enabled(record.metadata()) {
                log.log(record);
            }
        }
    }

    fn flush(&self) {
        if let Some(ref log) = self.dest_log {
            log.flush();
        }
    }
}
