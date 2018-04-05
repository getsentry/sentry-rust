//! Panic handler support.
//!
//! When the panic handler is registered with `register_panic_handler` a
//! Sentry critical error event will be emitted for all panics that
//! occur.
use std::panic;

use api::protocol::{Event, Exception, Level};
use utils::current_stacktrace;
use scope::with_client_and_scope;

/// Extract the message of a panic.
pub fn message_from_panic_info<'a>(info: &'a panic::PanicInfo) -> &'a str {
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
pub fn event_from_panic_info(info: &panic::PanicInfo) -> Event {
    let msg = message_from_panic_info(info);
    Event {
        exceptions: vec![
            Exception {
                ty: "panic".into(),
                value: Some(msg.to_string()),
                stacktrace: current_stacktrace(),
                ..Default::default()
            },
        ],
        level: Level::Critical,
        ..Default::default()
    }
}

/// Registes a panic handler that sends to sentry.
///
/// Optionally it can call into another panic handler.  To delegate to the
/// default panic handler one can do this:
///
/// ```rust,no_run
/// use std::panic;
/// use sentry::integrations::panic::register_panic_handler;
/// register_panic_handler(Some(panic::take_hook()));
/// ```
pub fn register_panic_handler<F>(callback: Option<F>)
where
    F: Fn(&panic::PanicInfo) + 'static + Sync + Send,
{
    panic::set_hook(Box::new(move |info| {
        with_client_and_scope(|client, scope| {
            client.capture_event(event_from_panic_info(info), Some(scope));
        });
        if let Some(cb) = callback.as_ref() {
            cb(info);
        }
    }));
}
