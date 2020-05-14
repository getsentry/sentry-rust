use std::error::Error;

use crate::protocol::{Event, Exception, Level};
use crate::utils::parse_type_name_from_debug;

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
    // We would ideally want to use `type_name_of_val`, because right now we
    // get `dyn Error` when just using `type_name`, but that is nightly only
    // for now.
    // See https://github.com/rust-lang/rust/issues/66359
    let (module, ty) = parse_type_name_from_debug(err);
    Exception {
        ty,
        module,
        value: Some(err.to_string()),
        ..Default::default()
    }
}
