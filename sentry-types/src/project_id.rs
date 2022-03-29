use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Raised if a project ID cannot be parsed from a string.
#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseProjectIdError {
    /// Raised if the value is not an integer in the supported range.
    #[error("invalid value for project id")]
    InvalidValue,
    /// Raised if an empty value is parsed.
    #[error("empty or missing project id")]
    EmptyValue,
}

/// Represents a project ID.
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
#[serde(into = "u64", from = "u64")]
pub struct ProjectId(String);

impl ProjectId {
    /// Creates a new project ID from its numeric value.
    #[inline]
    pub fn new(id: u64) -> Self {
        Self(id.to_string())
    }

    /// Returns the numeric value of this project id. None is returned if a
    /// valid could not be parsed from the project id.
    #[inline]
    pub fn value(&self) -> Option<u64> {
        self.0.parse::<u64>().ok()
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
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
            type Error = ParseProjectIdError;

            #[inline]
            fn try_from(val: $ty) -> Result<Self, Self::Error> {
                match u64::try_from(val) {
                    Ok(id) => Ok(Self::new(id)),
                    Err(_) => Err(ParseProjectIdError::InvalidValue),
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
    type Err = ParseProjectIdError;

    fn from_str(s: &str) -> Result<ProjectId, ParseProjectIdError> {
        if s.is_empty() {
            return Err(ParseProjectIdError::EmptyValue);
        }

        match s.parse::<u64>() {
            Ok(val) => Ok(ProjectId::new(val)),
            Err(_) => Err(ParseProjectIdError::InvalidValue),
        }
    }
}

// Combined with the serde into/from annotation, this allows the project ID to
// continue being serialized and deserialized as a u64 until other parts of
// sentry add in full support for project strings.
impl From<ProjectId> for u64 {
    fn from(pid: ProjectId) -> Self {
        match pid.value() {
            Some(val) => val,
            None => u64::MAX,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_basic_api() {
        let id: ProjectId = "42".parse().unwrap();
        assert_eq!(id, ProjectId::new(42));
        assert_eq!(
            "42xxx".parse::<ProjectId>(),
            Err(ParseProjectIdError::InvalidValue)
        );
        assert_eq!(
            "".parse::<ProjectId>(),
            Err(ParseProjectIdError::EmptyValue)
        );
        assert_eq!(ProjectId::new(42).to_string(), "42");

        assert_eq!(serde_json::to_string(&ProjectId::new(42)).unwrap(), "42");
        assert_eq!(
            serde_json::from_str::<ProjectId>("42").unwrap(),
            ProjectId::new(42)
        );
    }
}
