//! Sampling-related types.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::str::FromStr;
use thiserror::Error;

/// The number of decimal places in [`SampleRand`].
const SAMPLE_RAND_DECIMALS: usize = 6;

/// A random number generated at the start of a trace by the head of trace SDK.
///
/// The value is a number in the half-open interval [0.0, 1.0). Currently, at most six decimal
/// places can be represented, though this is subject to change.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct SampleRand {
    /// The value of the sample rand, in ticks, where one tick corresponds to `0.000001`.
    ///
    /// The wire representation is `0.` followed by the tick count padded to six digits (e.g.
    /// `0.5` normalizes to `"0.500000"`). The tick count is strictly less than 1,000,000, so
    /// the invalid value 1.0 is unrepresentable.
    ticks: u32,
}

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
        // Always rounds down; since `value < 1.0`, the result can never reach 1,000,000 ticks.
        Ok(Self {
            ticks: (value * 1_000_000.0).floor() as u32,
        })
    }
}

impl FromStr for SampleRand {
    type Err = InvalidSampleRandError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Self::try_from_spec_string(s) {
            Some(sample_rand) => Ok(sample_rand),
            // Fall back to f64 parsing for backwards-compatibility
            None => Self::try_from(
                s.parse::<f64>()
                    .map_err(InvalidSampleRandError::InvalidFloat)?,
            ),
        }
    }
}

impl Display for SampleRand {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // Always exactly six digits; the maximum output is `0.999999`.
        write!(f, "0.{:06}", self.ticks)
    }
}

impl SampleRand {
    /// Tries to parse a sample_rand from a (nearly) spec-compliant string.
    ///
    /// This will parse sample_rand that is formatted like `0.<digits>` without relying on float
    /// parsing. We can parse any number of digits, not just exactly six as per spec.
    ///
    /// Returns `None` if the string is not spec-compliant.
    fn try_from_spec_string(s: &str) -> Option<Self> {
        let digits = s.strip_prefix("0.")?;

        if digits.is_empty() || digits.bytes().any(|b| !b.is_ascii_digit()) {
            return None;
        }

        // The difference between the number of digits in the string and the number of
        // digits in a sample_rand value. After parsing the digits as a u32, we need to
        // multiply by 10 to this power in order to pad the value with enough zeroes.
        // If the incoming sample_rand value complies with the spec, missing_digits will
        // be zero because the incoming value has exactly six digits.
        let missing_digits: u32 = SAMPLE_RAND_DECIMALS
            .saturating_sub(digits.len())
            .try_into()
            .expect("this is at most six, which is representable as u32");

        // Truncate digits to at most six digits, parse as u32, and pad with missing
        // digits, if any.
        let ticks = digits[..SAMPLE_RAND_DECIMALS.min(digits.len())]
            .parse::<u32>()
            .ok()? // Should not currently error due to is_ascii_digit check
            .checked_mul(10_u32.pow(missing_digits))
            .expect("resulting value at most 999_999 which does not overflow");

        Some(Self { ticks })
    }
}
