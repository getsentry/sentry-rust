//! Sampling-related types.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::str::FromStr;
use thiserror::Error;

/// A random number generated at the start of a trace by the head of trace SDK.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct SampleRand(f64);

/// An error that indicates failure to construct a SampleRand.
#[derive(Debug, Error)]
pub enum InvalidSampleRandError {
    /// Indicates that the given value cannot be converted to a f64 succesfully.
    #[error("failed to parse f64: {0}")]
    InvalidFloat(#[from] std::num::ParseFloatError),

    /// Indicates that the given float is outside of the valid range for a sample rand, that is the
    /// half-open interval [0.0, 1.0).
    #[error("sample rand value out of admissible interval [0.0, 1.0)")]
    OutOfRange,
}

impl TryFrom<f64> for SampleRand {
    type Error = InvalidSampleRandError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !(0.0..1.0).contains(&value) {
            return Err(InvalidSampleRandError::OutOfRange);
        }
        Ok(Self(value))
    }
}

impl FromStr for SampleRand {
    type Err = InvalidSampleRandError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let x: f64 = s.parse().map_err(InvalidSampleRandError::InvalidFloat)?;
        Self::try_from(x)
    }
}

impl Display for SampleRand {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // Special case: "{:.6}" would round values greater than or equal to 0.9999995 to 1.0,
        // as Rust uses [rounding half-to-even](https://doc.rust-lang.org/std/fmt/#precision).
        // Round to 0.999999 instead to comply with spec.
        if self.0 >= 0.9999995 {
            write!(f, "0.999999")
        } else {
            write!(f, "{:.6}", self.0)
        }
    }
}
