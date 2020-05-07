//! Panic handler support.
//!
//! A panic handler can be installed that will automatically dispatch all errors
//! to Sentry that are caused by a panic.
//!
//! # Configuration
//!
//! The panic integration can be configured with an additional extractor, which
//! might optionally create a sentry `Event` out of a `PanicInfo`.
//!
//! ```
//! let integration = sentry_panic::PanicIntegration::default()
//!     .add_extractor(|info| None);
//! ```
//!
//! Additionally panics are forwarded to the previously registered panic hook.

use std::panic::{self, PanicInfo};
use std::sync::Once;

use sentry_core::backtrace_support::current_stacktrace;
use sentry_core::protocol::{Event, Exception, Level};
use sentry_core::{ClientOptions, Hub, Integration};

/// A panic handler that sends to Sentry.
///
/// This panic handler report panics to Sentry. It also attempts to prevent
/// double faults in some cases where it's known to be unsafe to invoke the
/// Sentry panic handler.
pub fn panic_handler(info: &PanicInfo<'_>) {
    Hub::with_active(|hub| {
        hub.with_integration(|integration: &PanicIntegration| {
            hub.capture_event(integration.event_from_panic_info(info))
        })
    });
}

pub type PanicExtractor = dyn Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync;

#[derive(Default)]
pub struct PanicIntegration {
    extractors: Vec<Box<PanicExtractor>>,
}

static INIT: Once = Once::new();

impl Integration for PanicIntegration {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn setup(&self, _cfg: &mut ClientOptions) {
        INIT.call_once(|| {
            let next = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                panic_handler(info);
                next(info);
            }));
        });
    }
}

/// Extract the message of a panic.
pub fn message_from_panic_info<'a>(info: &'a PanicInfo<'_>) -> &'a str {
    match info.payload().downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    }
}

impl PanicIntegration {
    /// Registers a new extractor.
    pub fn add_extractor<F>(mut self, f: F) -> Self
    where
        F: Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        self.extractors.push(Box::new(f));
        self
    }

    /// Creates an event from the given panic info.
    ///
    /// The stacktrace is calculated from the current frame.
    pub fn event_from_panic_info(&self, info: &PanicInfo<'_>) -> Event<'static> {
        for extractor in &self.extractors {
            if let Some(event) = extractor(info) {
                return event;
            }
        }

        let msg = message_from_panic_info(info);
        Event {
            exception: vec![Exception {
                ty: "panic".into(),
                value: Some(msg.to_string()),
                stacktrace: current_stacktrace(),
                ..Default::default()
            }]
            .into(),
            level: Level::Fatal,
            ..Default::default()
        }
    }
}
