//! Type definitions for Sentry metrics.

use std::fmt;

/// The unit of measurement of a metric value.
///
/// Units augment metric values by giving them a magnitude and semantics. There are certain types of
/// units that are subdivided in their precision, such as the [`DurationUnit`] for time
/// measurements.
///
/// Units and their precisions are uniquely represented by a string identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
pub enum MetricUnit {
    /// A time duration, defaulting to `"millisecond"`.
    Duration(DurationUnit),
    /// Size of information derived from bytes, defaulting to `"byte"`.
    Information(InformationUnit),
    /// Fractions such as percentages, defaulting to `"ratio"`.
    Fraction(FractionUnit),
    /// user-defined units without builtin conversion or default.
    Custom(CustomUnit),
    /// Untyped value without a unit (`""`).
    #[default]
    None,
}

impl MetricUnit {
    /// Returns `true` if the metric_unit is [`None`].
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl fmt::Display for MetricUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricUnit::Duration(u) => u.fmt(f),
            MetricUnit::Information(u) => u.fmt(f),
            MetricUnit::Fraction(u) => u.fmt(f),
            MetricUnit::Custom(u) => u.fmt(f),
            MetricUnit::None => f.write_str("none"),
        }
    }
}

impl std::str::FromStr for MetricUnit {
    type Err = ParseMetricUnitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "nanosecond" | "ns" => Self::Duration(DurationUnit::NanoSecond),
            "microsecond" => Self::Duration(DurationUnit::MicroSecond),
            "millisecond" | "ms" => Self::Duration(DurationUnit::MilliSecond),
            "second" | "s" => Self::Duration(DurationUnit::Second),
            "minute" => Self::Duration(DurationUnit::Minute),
            "hour" => Self::Duration(DurationUnit::Hour),
            "day" => Self::Duration(DurationUnit::Day),
            "week" => Self::Duration(DurationUnit::Week),

            "bit" => Self::Information(InformationUnit::Bit),
            "byte" => Self::Information(InformationUnit::Byte),
            "kilobyte" => Self::Information(InformationUnit::KiloByte),
            "kibibyte" => Self::Information(InformationUnit::KibiByte),
            "megabyte" => Self::Information(InformationUnit::MegaByte),
            "mebibyte" => Self::Information(InformationUnit::MebiByte),
            "gigabyte" => Self::Information(InformationUnit::GigaByte),
            "gibibyte" => Self::Information(InformationUnit::GibiByte),
            "terabyte" => Self::Information(InformationUnit::TeraByte),
            "tebibyte" => Self::Information(InformationUnit::TebiByte),
            "petabyte" => Self::Information(InformationUnit::PetaByte),
            "pebibyte" => Self::Information(InformationUnit::PebiByte),
            "exabyte" => Self::Information(InformationUnit::ExaByte),
            "exbibyte" => Self::Information(InformationUnit::ExbiByte),

            "ratio" => Self::Fraction(FractionUnit::Ratio),
            "percent" => Self::Fraction(FractionUnit::Percent),

            "" | "none" => Self::None,
            _ => Self::Custom(CustomUnit::parse(s)?),
        })
    }
}

/// Time duration units used in [`MetricUnit::Duration`].
///
/// Defaults to `millisecond`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DurationUnit {
    /// Nanosecond (`"nanosecond"`), 10^-9 seconds.
    NanoSecond,
    /// Microsecond (`"microsecond"`), 10^-6 seconds.
    MicroSecond,
    /// Millisecond (`"millisecond"`), 10^-3 seconds.
    MilliSecond,
    /// Full second (`"second"`).
    Second,
    /// Minute (`"minute"`), 60 seconds.
    Minute,
    /// Hour (`"hour"`), 3600 seconds.
    Hour,
    /// Day (`"day"`), 86,400 seconds.
    Day,
    /// Week (`"week"`), 604,800 seconds.
    Week,
}

impl Default for DurationUnit {
    fn default() -> Self {
        Self::MilliSecond
    }
}

impl fmt::Display for DurationUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NanoSecond => f.write_str("nanosecond"),
            Self::MicroSecond => f.write_str("microsecond"),
            Self::MilliSecond => f.write_str("millisecond"),
            Self::Second => f.write_str("second"),
            Self::Minute => f.write_str("minute"),
            Self::Hour => f.write_str("hour"),
            Self::Day => f.write_str("day"),
            Self::Week => f.write_str("week"),
        }
    }
}

/// An error parsing a [`MetricUnit`] or one of its variants.
#[derive(Clone, Copy, Debug)]
pub struct ParseMetricUnitError(());

/// Size of information derived from bytes, used in [`MetricUnit::Information`].
///
/// Defaults to `byte`. See also [Units of
/// information](https://en.wikipedia.org/wiki/Units_of_information).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum InformationUnit {
    /// Bit (`"bit"`), corresponding to 1/8 of a byte.
    ///
    /// Note that there are computer systems with a different number of bits per byte.
    Bit,
    /// Byte (`"byte"`).
    Byte,
    /// Kilobyte (`"kilobyte"`), 10^3 bytes.
    KiloByte,
    /// Kibibyte (`"kibibyte"`), 2^10 bytes.
    KibiByte,
    /// Megabyte (`"megabyte"`), 10^6 bytes.
    MegaByte,
    /// Mebibyte (`"mebibyte"`), 2^20 bytes.
    MebiByte,
    /// Gigabyte (`"gigabyte"`), 10^9 bytes.
    GigaByte,
    /// Gibibyte (`"gibibyte"`), 2^30 bytes.
    GibiByte,
    /// Terabyte (`"terabyte"`), 10^12 bytes.
    TeraByte,
    /// Tebibyte (`"tebibyte"`), 2^40 bytes.
    TebiByte,
    /// Petabyte (`"petabyte"`), 10^15 bytes.
    PetaByte,
    /// Pebibyte (`"pebibyte"`), 2^50 bytes.
    PebiByte,
    /// Exabyte (`"exabyte"`), 10^18 bytes.
    ExaByte,
    /// Exbibyte (`"exbibyte"`), 2^60 bytes.
    ExbiByte,
}

impl Default for InformationUnit {
    fn default() -> Self {
        Self::Byte
    }
}

impl fmt::Display for InformationUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bit => f.write_str("bit"),
            Self::Byte => f.write_str("byte"),
            Self::KiloByte => f.write_str("kilobyte"),
            Self::KibiByte => f.write_str("kibibyte"),
            Self::MegaByte => f.write_str("megabyte"),
            Self::MebiByte => f.write_str("mebibyte"),
            Self::GigaByte => f.write_str("gigabyte"),
            Self::GibiByte => f.write_str("gibibyte"),
            Self::TeraByte => f.write_str("terabyte"),
            Self::TebiByte => f.write_str("tebibyte"),
            Self::PetaByte => f.write_str("petabyte"),
            Self::PebiByte => f.write_str("pebibyte"),
            Self::ExaByte => f.write_str("exabyte"),
            Self::ExbiByte => f.write_str("exbibyte"),
        }
    }
}

/// Units of fraction used in [`MetricUnit::Fraction`].
///
/// Defaults to `ratio`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FractionUnit {
    /// Floating point fraction of `1`.
    Ratio,
    /// Ratio expressed as a fraction of `100`. `100%` equals a ratio of `1.0`.
    Percent,
}

impl Default for FractionUnit {
    fn default() -> Self {
        Self::Ratio
    }
}

impl fmt::Display for FractionUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ratio => f.write_str("ratio"),
            Self::Percent => f.write_str("percent"),
        }
    }
}

const CUSTOM_UNIT_MAX_SIZE: usize = 15;

/// Custom user-defined units without builtin conversion.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct CustomUnit([u8; CUSTOM_UNIT_MAX_SIZE]);

impl CustomUnit {
    /// Parses a `CustomUnit` from a string.
    pub fn parse(s: &str) -> Result<Self, ParseMetricUnitError> {
        if !s.is_ascii() {
            return Err(ParseMetricUnitError(()));
        }

        let mut unit = Self([0; CUSTOM_UNIT_MAX_SIZE]);
        let slice = unit.0.get_mut(..s.len()).ok_or(ParseMetricUnitError(()))?;
        slice.copy_from_slice(s.as_bytes());
        unit.0.make_ascii_lowercase();
        Ok(unit)
    }

    /// Returns the string representation of this unit.
    #[inline]
    pub fn as_str(&self) -> &str {
        // Safety: The string is already validated to be valid ASCII when
        // parsing `CustomUnit`.
        unsafe { std::str::from_utf8_unchecked(&self.0).trim_end_matches('\0') }
    }
}

impl fmt::Debug for CustomUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl fmt::Display for CustomUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl std::str::FromStr for CustomUnit {
    type Err = ParseMetricUnitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl std::ops::Deref for CustomUnit {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_unit_parse() {
        assert_eq!("foo", CustomUnit::parse("Foo").unwrap().as_str());
        assert_eq!(
            "0123456789abcde",
            CustomUnit::parse("0123456789abcde").unwrap().as_str()
        );
        assert!(CustomUnit::parse("this_is_a_unit_that_is_too_long").is_err());
    }
}
