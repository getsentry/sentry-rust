// NOTE: This is still a nightly-only feature.
// See https://github.com/rust-lang/rust/issues/66359
//use std::any::type_name_of_val;
use std::error::Error;

use crate::protocol::{Event, Exception, Level};

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
/// assert_eq!(event.exception[0].value, Some("inner".into()));
/// assert_eq!(event.exception[1].value, Some("outer".into()));
/// ```
pub fn event_from_error(mut err: &dyn Error) -> Event<'static> {
    let mut exceptions = vec![Exception {
        ty: String::from("Error"),
        value: Some(err.to_string()),
        ..Default::default()
    }];

    while let Some(source) = err.source() {
        exceptions.push(Exception {
            ty: String::from("Error"),
            value: Some(source.to_string()),
            ..Default::default()
        });
        err = source;
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: Level::Error,
        ..Default::default()
    }
}
