use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

use failure::Fail;

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

/// Represents a project ID.
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct ProjectId(u64);

impl ProjectId {
    /// Creates a new project ID from its numeric value.
    #[inline]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the numeric value of this project id.
    #[inline]
    pub fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}

impl fmt::Debug for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}

macro_rules! impl_from {
    ($ty:ty) => {
        impl From<$ty> for ProjectId {
            #[inline]
            fn from(val: $ty) -> Self {
                Self::new(val as u64)
            }
        }
    };
}

impl_from!(u8);
impl_from!(u16);
impl_from!(u32);
impl_from!(u64);

macro_rules! impl_try_from {
    ($ty:ty) => {
        impl TryFrom<$ty> for ProjectId {
            type Error = ProjectIdParseError;

            #[inline]
            fn try_from(val: $ty) -> Result<Self, Self::Error> {
                match u64::try_from(val) {
                    Ok(id) => Ok(Self::new(id)),
                    Err(_) => Err(ProjectIdParseError::InvalidValue),
                }
            }
        }
    };
}

impl_try_from!(usize);
impl_try_from!(i8);
impl_try_from!(i16);
impl_try_from!(i32);
impl_try_from!(i64);

impl FromStr for ProjectId {
    type Err = ProjectIdParseError;

    fn from_str(s: &str) -> Result<ProjectId, ProjectIdParseError> {
        if s.is_empty() {
            return Err(ProjectIdParseError::EmptyValue);
        }

        match s.parse::<u64>() {
            Ok(val) => Ok(ProjectId::new(val)),
            Err(_) => Err(ProjectIdParseError::InvalidValue),
        }
    }
}

impl_str_serde!(ProjectId);

#[cfg(test)]
mod test {
    use super::*;
    use serde_json;

    #[test]
    fn test_basic_api() {
        let id: ProjectId = "42".parse().unwrap();
        assert_eq!(id, ProjectId::new(42));
        assert_eq!(
            "42xxx".parse::<ProjectId>(),
            Err(ProjectIdParseError::InvalidValue)
        );
        assert_eq!(
            "".parse::<ProjectId>(),
            Err(ProjectIdParseError::EmptyValue)
        );
        assert_eq!(ProjectId::new(42).to_string(), "42");

        assert_eq!(
            serde_json::to_string(&ProjectId::new(42)).unwrap(),
            "\"42\""
        );
        assert_eq!(
            serde_json::from_str::<ProjectId>("\"42\"").unwrap(),
            ProjectId::new(42)
        );
    }
}
