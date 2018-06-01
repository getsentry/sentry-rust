use std::fmt;
use std::str::FromStr;

/// Represents a project ID.
///
/// This is a thin wrapper around IDs supported by the Sentry
/// server.  The idea is that the sentry server generally can
/// switch the ID format in the future (eg: we implement the IDs
/// as strings and not as integers) but the actual ID format that
/// is encountered are currently indeed integers.
///
/// To be future proof we support either integers or "short"
/// strings.
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct ProjectId {
    // for now the only supported format is indeed an u64
    val: u64,
}

/// Raised if a project ID cannot be parsed from a string.
#[derive(Debug, Fail, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectIdParseError {
    /// Raised if the value is not an integer in the supported range.
    #[fail(display = "invalid value for project id")]
    InvalidValue,
    /// Raised if an empty value is parsed.
    #[fail(display = "empty or missing project id")]
    EmptyValue,
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.val)
    }
}

impl fmt::Debug for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

macro_rules! impl_from {
    ($ty:ty) => {
        impl From<$ty> for ProjectId {
            fn from(val: $ty) -> ProjectId {
                ProjectId { val: val as u64 }
            }
        }
    };
}

impl_from!(usize);
impl_from!(u8);
impl_from!(u16);
impl_from!(u32);
impl_from!(u64);
impl_from!(i8);
impl_from!(i16);
impl_from!(i32);
impl_from!(i64);

impl FromStr for ProjectId {
    type Err = ProjectIdParseError;

    fn from_str(s: &str) -> Result<ProjectId, ProjectIdParseError> {
        if s.is_empty() {
            return Err(ProjectIdParseError::EmptyValue);
        }
        match s.parse::<u64>() {
            Ok(val) => Ok(ProjectId { val }),
            Err(_) => Err(ProjectIdParseError::InvalidValue),
        }
    }
}

impl_str_serialization!(ProjectId);

#[cfg(test)]
mod test {
    use super::*;
    use serde_json;

    #[test]
    fn test_basic_api() {
        let id: ProjectId = "42".parse().unwrap();
        assert_eq!(id, ProjectId::from(42));
        assert_eq!(
            "42xxx".parse::<ProjectId>(),
            Err(ProjectIdParseError::InvalidValue)
        );
        assert_eq!(
            "".parse::<ProjectId>(),
            Err(ProjectIdParseError::EmptyValue)
        );
        assert_eq!(ProjectId::from(42).to_string(), "42");

        assert_eq!(
            serde_json::to_string(&ProjectId::from(42)).unwrap(),
            "\"42\""
        );
        assert_eq!(
            serde_json::from_str::<ProjectId>("\"42\"").unwrap(),
            ProjectId::from(42)
        );
    }
}
