//! Useful utilities for working with events.

/// Parse the types name from `Debug` output.
///
/// # Examples
///
/// ```
/// use sentry_core::utils::parse_type_from_debug;
///
/// let err = "NaN".parse::<usize>().unwrap_err();
/// assert_eq!(&parse_type_from_debug(&err), "ParseIntError");
/// ```
pub fn parse_type_from_debug<D: std::fmt::Debug + ?Sized>(d: &D) -> String {
    let dbg = format!("{:#?}", d);

    dbg.split(&[' ', '(', '{', '\r', '\n'][..])
        .next()
        .unwrap_or(&dbg)
        .trim()
        .to_owned()
}

#[test]
fn test_parse_type_from_debug() {
    use parse_type_from_debug as parse;
    #[derive(Debug)]
    struct MyStruct;
    assert_eq!(&parse(&MyStruct), "MyStruct");

    let err = "NaN".parse::<usize>().unwrap_err();
    assert_eq!(&parse(&err), "ParseIntError");

    let err = anyhow::Error::from(err);
    assert_eq!(&parse(&err), "ParseIntError");

    let err = sentry_types::ParseDsnError::from(sentry_types::ParseProjectIdError::EmptyValue);
    assert_eq!(&parse(&err), "InvalidProjectId");
}
