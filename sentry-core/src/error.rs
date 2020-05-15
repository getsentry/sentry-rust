use std::error::Error;

use sentry_types::Uuid;

use crate::protocol::{Event, Exception, Level};
use crate::utils::parse_type_from_debug;
use crate::Hub;

impl Hub {
    /// Capture any `std::error::Error`.
    pub fn capture_error<E: Error + ?Sized>(&self, error: &E) -> sentry_types::Uuid {
        with_client_impl! {{
            self.inner.with(|stack| {
                let top = stack.top();
                if top.client.is_some() {
                    let event = event_from_error(error);
                    self.capture_event(event)
                } else {
                    Uuid::nil()
                }
            })
        }}
    }
}

/// Captures a `std::error::Error`.
///
/// Creates an event from the given error and sends it to the current hub.
/// A chain of errors will be resolved as well, and sorted oldest to newest, as
/// described on https://develop.sentry.dev/sdk/event-payloads/exception/.
///
/// # Examples
/// ```
/// # use sentry_core as sentry;
/// sentry::capture_error(&std::io::Error::last_os_error());
/// ```
#[allow(unused_variables)]
pub fn capture_error<E: Error + ?Sized>(error: &E) -> sentry_types::Uuid {
    Hub::with_active(|hub| hub.capture_error(error))
}

/// Create a sentry `Event` from a `std::error::Error`.
///
/// A chain of errors will be resolved as well, and sorted oldest to newest, as
/// described on https://develop.sentry.dev/sdk/event-payloads/exception/.
///
/// # Examples
///
/// ```
/// use thiserror::Error;
///
/// #[derive(Debug, Error)]
/// #[error("inner")]
/// struct InnerError;
///
/// #[derive(Debug, Error)]
/// #[error("outer")]
/// struct OuterError(#[from] InnerError);
///
/// let event = sentry_core::event_from_error(&OuterError(InnerError));
/// assert_eq!(event.level, sentry_core::protocol::Level::Error);
/// assert_eq!(event.exception.len(), 2);
/// assert_eq!(&event.exception[0].ty, "InnerError");
/// assert_eq!(event.exception[0].value, Some("inner".into()));
/// assert_eq!(&event.exception[1].ty, "OuterError");
/// assert_eq!(event.exception[1].value, Some("outer".into()));
/// ```
pub fn event_from_error<E: Error + ?Sized>(err: &E) -> Event<'static> {
    let mut exceptions = vec![exception_from_error(err)];

    let mut source = err.source();
    while let Some(err) = source {
        exceptions.push(exception_from_error(err));
        source = err.source();
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: Level::Error,
        ..Default::default()
    }
}

fn exception_from_error<E: Error + ?Sized>(err: &E) -> Exception {
    Exception {
        ty: parse_type_from_debug(err),
        value: Some(err.to_string()),
        ..Default::default()
    }
}
