//! The Sentry Panic handler integration.
//!
//! The `PanicIntegration`, which is enabled by default in `sentry`, installs a
//! panic handler that will automatically dispatch all errors to Sentry that
//! are caused by a panic.
//! Additionally, panics are forwarded to the previously registered panic hook.
//!
//! # Configuration
//!
//! The panic integration can be configured with an additional extractor, which
//! might optionally create a sentry `Event` out of a `PanicInfo`.
//!
//! ```
//! let integration = sentry_panic::PanicIntegration::default().add_extractor(|info| None);
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

#[allow(deprecated)] // `PanicHookInfo` is only available in Rust 1.81+.
use std::panic::{self, PanicInfo};
use std::sync::Once;

use sentry_backtrace::current_stacktrace;
use sentry_core::protocol::{Event, Exception, Level, Mechanism};
use sentry_core::{ClientOptions, Integration, sentry_debug};

/// A panic handler that sends to Sentry.
///
/// This panic handler reports panics to Sentry. It also attempts to prevent
/// double faults in some cases where it's known to be unsafe to invoke the
/// Sentry panic handler.
#[allow(deprecated)] // `PanicHookInfo` is only available in Rust 1.81+.
pub fn panic_handler(info: &PanicInfo<'_>) {
    sentry_debug!("[PanicIntegration] Panic detected: {}", message_from_panic_info(info));
    sentry_core::with_integration(|integration: &PanicIntegration, hub| {
        let event = integration.event_from_panic_info(info);
        sentry_debug!("[PanicIntegration] Created event {} for panic", event.event_id);
        hub.capture_event(event);
        if let Some(client) = hub.client() {
            sentry_debug!("[PanicIntegration] Flushing client after panic");
            client.flush(None);
        }
    });
}

#[allow(deprecated)] // `PanicHookInfo` is only available in Rust 1.81+.
type PanicExtractor = dyn Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync;

/// The Sentry Panic handler Integration.
#[derive(Default)]
pub struct PanicIntegration {
    extractors: Vec<Box<PanicExtractor>>,
}

impl std::fmt::Debug for PanicIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanicIntegration")
            .field("extractors", &self.extractors.len())
            .finish()
    }
}

static INIT: Once = Once::new();

impl Integration for PanicIntegration {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn setup(&self, _cfg: &mut ClientOptions) {
        sentry_debug!("[PanicIntegration] Setting up panic handler");
        INIT.call_once(|| {
            sentry_debug!("[PanicIntegration] Installing panic hook (one-time setup)");
            let next = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                panic_handler(info);
                next(info);
            }));
        });
    }
}

/// Extract the message of a panic.
#[allow(deprecated)] // `PanicHookInfo` is only available in Rust 1.81+.
pub fn message_from_panic_info<'a>(info: &'a PanicInfo<'_>) -> &'a str {
    match info.payload().downcast_ref::<&'static str>() {
        Some(s) => s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    }
}

impl PanicIntegration {
    /// Creates a new Panic Integration.
    pub fn new() -> Self {
        sentry_debug!("[PanicIntegration] Creating new panic integration");
        Self::default()
    }

    /// Registers a new extractor.
    #[must_use]
    #[allow(deprecated)] // `PanicHookInfo` is only available in Rust 1.81+.
    pub fn add_extractor<F>(mut self, f: F) -> Self
    where
        F: Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        sentry_debug!("[PanicIntegration] Adding custom panic extractor");
        self.extractors.push(Box::new(f));
        self
    }

    /// Creates an event from the given panic info.
    ///
    /// The stacktrace is calculated from the current frame.
    #[allow(deprecated)] // `PanicHookInfo` is only available in Rust 1.81+.
    pub fn event_from_panic_info(&self, info: &PanicInfo<'_>) -> Event<'static> {
        sentry_debug!("[PanicIntegration] Creating event from panic info, {} extractors available", self.extractors.len());
        
        for extractor in &self.extractors {
            if let Some(event) = extractor(info) {
                sentry_debug!("[PanicIntegration] Custom extractor created event");
                return event;
            }
        }

        sentry_debug!("[PanicIntegration] Using default panic event creation");

        // TODO: We would ideally want to downcast to `std::error:Error` here
        // and use `event_from_error`, but that way we won't get meaningful
        // backtraces yet.

        let msg = message_from_panic_info(info);
        Event {
            exception: vec![Exception {
                ty: "panic".into(),
                mechanism: Some(Mechanism {
                    ty: "panic".into(),
                    handled: Some(false),
                    ..Default::default()
                }),
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
