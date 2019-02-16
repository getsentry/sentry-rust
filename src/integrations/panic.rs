//! Panic handler support.
//!
//! **Feature:** `with_panic` (enabled by default)
//!
//! A panic handler can be installed that will automatically dispatch all errors
//! to Sentry that are caused by a panic.
//!
//! # Configuration
//!
//! ```no_run
//! use sentry::integrations::panic::register_panic_handler;
//! register_panic_handler();
//! ```
//!
//! Additionally panics are forwarded to the previously registered panic hook.
use std::panic::{self, PanicInfo};

use crate::backtrace_support::current_stacktrace;
use crate::hub::Hub;
use crate::protocol::{Event, Exception, Level};

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

/// Creates an event from the given panic info.
///
/// The stacktrace is calculated from the current frame.
pub fn event_from_panic_info(info: &PanicInfo<'_>) -> Event<'static> {
    if cfg!(feature = "with_failure") {
        use failure::Error;
        use crate::integrations::failure::event_from_error;

        if let Some(e) = info.payload().downcast_ref::<Error>() {
            return Event {
                level: Level::Fatal,
                ..event_from_error(e)
            };
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

/// A panic handler that sends to Sentry.
///
/// This panic handler report panics to Sentry.  It also attempts to prevent
/// double faults in some cases where it's known to be unsafe to invoke the
/// Sentry panic handler.
pub fn panic_handler(info: &PanicInfo<'_>) {
    Hub::with_active(|hub| {
        hub.capture_event(event_from_panic_info(info));
    });
}

/// Registes the panic handler.
///
/// This registers the panic handler (`panic_handler`) as panic hook and
/// dispatches automatically to the one that was there before.
///
/// ```
/// use sentry::integrations::panic::register_panic_handler;
/// register_panic_handler();
/// ```
pub fn register_panic_handler() {
    let next = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        panic_handler(info);
        next(info);
    }));
}
