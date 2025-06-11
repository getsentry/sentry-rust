use std::thread;

use sentry_core::protocol::{Event, Thread};
use sentry_core::{ClientOptions, Integration, sentry_debug};

use crate::current_stacktrace;
use crate::process::process_event_stacktrace;

/// Integration to process Event stacktraces.
///
/// This integration will trim backtraces, depending on the `trim_backtraces`
/// and `extra_border_frames` options.
/// It will then classify each frame according to the `in_app_include` and
/// `in_app_exclude` options.
#[derive(Debug, Default)]
pub struct ProcessStacktraceIntegration;

impl ProcessStacktraceIntegration {
    /// Creates a new Integration to process stacktraces.
    pub fn new() -> Self {
        Self
    }
}

impl Integration for ProcessStacktraceIntegration {
    fn name(&self) -> &'static str {
        "process-stacktrace"
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        options: &ClientOptions,
    ) -> Option<Event<'static>> {
        sentry_debug!("[ProcessStacktraceIntegration] Processing event {}", event.event_id);
        
        let mut processed_stacks = 0;
        
        for exc in &mut event.exception {
            if let Some(ref mut stacktrace) = exc.stacktrace {
                process_event_stacktrace(stacktrace, options);
                processed_stacks += 1;
            }
        }
        for th in &mut event.threads {
            if let Some(ref mut stacktrace) = th.stacktrace {
                process_event_stacktrace(stacktrace, options);
                processed_stacks += 1;
            }
        }
        if let Some(ref mut stacktrace) = event.stacktrace {
            process_event_stacktrace(stacktrace, options);
            processed_stacks += 1;
        }
        
        if processed_stacks > 0 {
            sentry_debug!("[ProcessStacktraceIntegration] Processed {} stacktraces", processed_stacks);
        }
        
        Some(event)
    }
}

/// Integration to attach stacktraces to Events.
///
/// This integration will add an additional thread backtrace to captured
/// messages, respecting the `attach_stacktrace` option.
#[derive(Debug, Default)]
pub struct AttachStacktraceIntegration;

impl AttachStacktraceIntegration {
    /// Creates a new Integration to attach stacktraces to Events.
    pub fn new() -> Self {
        Self
    }
}

impl Integration for AttachStacktraceIntegration {
    fn name(&self) -> &'static str {
        "attach-stacktrace"
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        options: &ClientOptions,
    ) -> Option<Event<'static>> {
        sentry_debug!("[AttachStacktraceIntegration] Processing event {}", event.event_id);
        
        if options.attach_stacktrace && !has_stacktrace(&event) {
            sentry_debug!("[AttachStacktraceIntegration] Event has no stacktrace, attaching current thread stacktrace");
            let thread = current_thread(true);
            if thread.stacktrace.is_some() {
                event.threads.values.push(thread);
                sentry_debug!("[AttachStacktraceIntegration] Attached stacktrace to event");
            } else {
                sentry_debug!("[AttachStacktraceIntegration] Failed to capture current stacktrace");
            }
        } else if !options.attach_stacktrace {
            sentry_debug!("[AttachStacktraceIntegration] attach_stacktrace is disabled");
        } else {
            sentry_debug!("[AttachStacktraceIntegration] Event already has stacktrace, not attaching");
        }
        Some(event)
    }
}

fn has_stacktrace(event: &Event) -> bool {
    event.stacktrace.is_some()
        || event.exception.iter().any(|exc| exc.stacktrace.is_some())
        || event.threads.iter().any(|thrd| thrd.stacktrace.is_some())
}

/// Captures information about the current thread.
///
/// If `with_stack` is set to `true` the current stacktrace is
/// attached.
pub fn current_thread(with_stack: bool) -> Thread {
    // NOTE: `as_u64` is nightly only
    // See https://github.com/rust-lang/rust/issues/67939
    let thread_id: u64 = unsafe { std::mem::transmute(thread::current().id()) };
    Thread {
        id: Some(thread_id.to_string().into()),
        name: thread::current().name().map(str::to_owned),
        current: true,
        stacktrace: if with_stack {
            current_stacktrace()
        } else {
            None
        },
        ..Default::default()
    }
}
