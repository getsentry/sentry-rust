use log::{Level, LevelFilter, Record};

#[derive(Clone, Copy, Debug)]
pub struct Filters {
    pub global_filter: Option<LevelFilter>,
    pub filter: LevelFilter,
    pub emit_breadcrumbs: bool,
    pub emit_error_events: bool,
    pub emit_warning_events: bool,
    pub attach_stacktraces: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            global_filter: None,
            filter: LevelFilter::Info,
            emit_breadcrumbs: true,
            emit_error_events: true,
            emit_warning_events: false,
            attach_stacktraces: true,
        }
    }
}

impl Filters {
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
    use crate::LogIntegration;

    let opt_warn = LogIntegration {
        filter: LevelFilter::Warn,
        ..Default::default()
    }
    .create_filters();
    assert_eq!(opt_warn.effective_global_filter(), LevelFilter::Warn);
    assert_eq!(opt_warn.issue_filter(), LevelFilter::Error);

    let opt_debug = LogIntegration {
        global_filter: Some(LevelFilter::Debug),
        filter: LevelFilter::Warn,
        ..Default::default()
    }
    .create_filters();
    assert_eq!(opt_debug.effective_global_filter(), LevelFilter::Debug);

    let opt_debug_inverse = LogIntegration {
        global_filter: Some(LevelFilter::Warn),
        filter: LevelFilter::Debug,
        ..Default::default()
    }
    .create_filters();
    assert_eq!(
        opt_debug_inverse.effective_global_filter(),
        LevelFilter::Debug
    );

    let opt_weird = LogIntegration {
        filter: LevelFilter::Error,
        emit_warning_events: true,
        ..Default::default()
    }
    .create_filters();
    assert_eq!(opt_weird.issue_filter(), LevelFilter::Warn);
    assert_eq!(opt_weird.effective_global_filter(), LevelFilter::Warn);
}
