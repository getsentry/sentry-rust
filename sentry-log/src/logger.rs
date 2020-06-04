use crate::converters::{breadcrumb_from_record, event_from_record};
use crate::LogIntegration;

/// Provides a dispatching logger.
#[derive(Default)]
pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, md: &log::Metadata<'_>) -> bool {
        sentry_core::with_integration(|integration: &LogIntegration, _| {
            if let Some(global_filter) = integration.global_filter {
                if md.level() > global_filter {
                    return false;
                }
            }
            md.level() <= integration.filter
                || integration
                    .dest_log
                    .as_ref()
                    .map_or(false, |x| x.enabled(md))
        })
    }

    fn log(&self, record: &log::Record<'_>) {
        sentry_core::with_integration(|integration: &LogIntegration, hub| {
            if integration.create_issue_for_record(record) {
                hub.capture_event(event_from_record(record, integration.attach_stacktraces));
            }
            if integration.emit_breadcrumbs && record.level() <= integration.filter {
                sentry_core::add_breadcrumb(|| breadcrumb_from_record(record));
            }
            if let Some(ref log) = integration.dest_log {
                if log.enabled(record.metadata()) {
                    log.log(record);
                }
            }
        })
    }

    fn flush(&self) {
        sentry_core::with_integration(|integration: &LogIntegration, _| {
            if let Some(ref log) = integration.dest_log {
                log.flush();
            }
        })
    }
}
