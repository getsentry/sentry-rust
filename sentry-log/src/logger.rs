use sentry_core::{add_breadcrumb, Hub};

use crate::converters::{breadcrumb_from_record, event_from_record};
use crate::integration::with_integration;

/// Provides a dispatching logger.
#[derive(Default)]
pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, md: &log::Metadata<'_>) -> bool {
        with_integration(|integration| {
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
        with_integration(|integration| {
            if integration.create_issue_for_record(record) {
                Hub::with_active(|hub| {
                    hub.capture_event(event_from_record(record, integration.attach_stacktraces))
                });
            }
            if integration.emit_breadcrumbs && record.level() <= integration.filter {
                add_breadcrumb(|| breadcrumb_from_record(record))
            }
            if let Some(ref log) = integration.dest_log {
                if log.enabled(record.metadata()) {
                    log.log(record);
                }
            }
        })
    }

    fn flush(&self) {
        with_integration(|integration| {
            if let Some(ref log) = integration.dest_log {
                log.flush();
            }
        })
    }
}
