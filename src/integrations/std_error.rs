//! Adds support for `std::error::Error`.
//!
//! **Feature:** `with_std_error`
//!
//! # Example
//!
//! ```no_run
//! # extern crate sentry;
//! # #[derive(Debug)]
//! # struct MyError;
//! # impl std::fmt::Display for MyError {
//! # fn fmt(&self, _: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }
//! # }
//! # impl std::error::Error for MyError {}
//! # fn function_that_might_fail() -> Result<(), MyError> { Ok(()) }
//! use sentry::integrations::std_error::capture_error;
//! # fn test() -> Result<(), MyError> {
//! let result = match function_that_might_fail() {
//!     Ok(result) => result,
//!     Err(err) => {
//!         capture_error(&err);
//!         return Err(err);
//!     }
//! };
//! # Ok(()) }
//! # fn main() { test().unwrap() }

use std::error::Error;

use crate::hub::Hub;
use crate::internals::Uuid;
use crate::protocol::{Event, Exception, Level};

fn parse_type_name(tn: &str) -> (Option<String>, String) {
    // While standard library guarantees little about format of
    // std::any::type_name(), we can assume it contains the type's name and
    // possibly its module path and generic parameters, likely formatted as
    // path::Name<Generics>.

    let mut first_bracket = tn.len();

    // If tn ends with a '>' then it's likely it a generic type. In this case we
    // don't know which '::' is the last '::' separating module path from type's
    // name since there may be '::' in generics, such as in
    // `Option<std::string::String>`. This fragment tries to find the opening
    // bracket of the generic block.
    if tn.ends_with('>') {
        // Number of opened brackets.
        let mut count = 0;
        let mut end = tn.len();

        while let Some(inx) = tn[..end].rfind(&['<', '>'] as &[char]) {
            end = inx;
            let chr = tn[inx..].chars().next().unwrap();

            // There are more opening brackets than closing brackets.
            if chr == '<' && count == 0 {
                break;
            }

            // Found the first opening bracket.
            if chr == '<' && count == 1 {
                first_bracket = inx;
                break;
            }

            if chr == '<' {
                // Found an opening bracket.
                count -= 1;
            } else {
                // Found a closing bracket.
                count += 1;
            }
        }
    }

    // At this point first_bracket point to either the end of tn, or to the
    // first character of what we believe is a generic parameter block. We can
    // expect then, that the last '::' before first_bracket separates module
    // path from type name.

    match tn[..first_bracket].rfind("::") {
        // ::Name
        Some(0) => (None, tn[2..].to_string()),
        // path::Name
        Some(inx) => (Some(tn[..inx].to_string()), tn[inx + 2..].to_string()),
        // Name
        None => (None, tn.to_string()),
    }
}

fn error_typename<E: ?Sized>() -> (Option<String>, String) {
    parse_type_name(std::any::type_name::<E>())
}

/// This converts a single error instance into an exception.
///
/// This is typically not very useful as the `event_from_error` method will
/// assemble an entire event with all the causes of an error, however for
/// certain more complex situations where errors are contained within a non
/// error error type that might also carry useful information it can be useful
/// to call this method instead.
pub fn exception_from_single_error<E: Error + ?Sized>(e: &E) -> Exception {
    let (module, ty) = error_typename::<E>();
    Exception {
        ty,
        module,
        value: Some(e.to_string()),
        // TODO: backtrace
        ..Default::default()
    }
}

/// Helper function to create an event from a `std::error::Error`.
pub fn event_from_error<E: Error + ?Sized>(err: &E) -> Event<'static> {
    let mut exceptions = vec![exception_from_single_error(err)];

    let mut ptr: Option<&dyn Error> = None;
    while let Some(source) = ptr.map(Error::source).unwrap_or_else(|| err.source()) {
        exceptions.push(exception_from_single_error(source));
        ptr = Some(source);
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: Level::Error,
        ..Default::default()
    }
}

/// Captures a `std::error::Error`.
///
/// This dispatches to the current hub.
pub fn capture_error<E: Error + ?Sized>(err: &E) -> Uuid {
    Hub::with_active(|hub| hub.capture_error(err))
}

/// Hub extension methods for working with errors.
pub trait ErrorHubExt {
    /// Captures a `std::error::Error`.
    fn capture_error<E: Error + ?Sized>(&self, err: &E) -> Uuid;
}

impl ErrorHubExt for Hub {
    fn capture_error<E: Error + ?Sized>(&self, err: &E) -> Uuid {
        self.capture_event(event_from_error(err))
    }
}

#[test]
fn test_parse_typename() {
    assert_eq!(parse_type_name("JustName"), (None, "JustName".into()));
    assert_eq!(
        parse_type_name("With<Generics>"),
        (None, "With<Generics>".into()),
    );
    assert_eq!(
        parse_type_name("with::module::Path"),
        (Some("with::module".into()), "Path".into()),
    );
    assert_eq!(
        parse_type_name("with::module::Path<and::Generics>"),
        (Some("with::module".into()), "Path<and::Generics>".into()),
    );
}
