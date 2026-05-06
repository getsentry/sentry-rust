//! Contains a [`Unit`] type, which encodes all
//! [units we support](https://develop.sentry.dev/sdk/foundations/state-management/scopes/attributes/#units).
//! Implementations to convert from common string types are provided.

use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize};

/// A unit for a metric.
///
/// Recognized units are explicitly enumerated, while other units can be set using the
/// [`Unit::Other`] variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all(serialize = "lowercase"))]
#[expect(missing_docs)] // Most of the variants are self-explanatory
#[non_exhaustive]
pub enum Unit {
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Bit,
    Byte,
    Kilobyte,
    Kibibyte,
    Megabyte,
    Mebibyte,
    Gigabyte,
    Gibibyte,
    Terabyte,
    Tebibyte,
    Petabyte,
    Pebibyte,
    Exabyte,
    Exbibyte,
    Ratio,
    Percent,
    /// Any other unit, which may not be recognized by the Sentry UI.
    ///
    /// We advise against constructing this variant directly; instead, rely on the `From`
    /// implementations to convert from a `String`, `Cow<'static, str>`, or `&'static str`,
    /// as these implementations normalize to the known units, including to any units we
    /// may add in the future as we add them.
    #[serde(untagged)]
    Other(Cow<'static, str>),
}

impl From<Cow<'static, str>> for Unit {
    /// Convert a [`Cow<'static, str>`] to a [`Unit`]. Known units (including standard symbols,
    /// such as "MB" for "megabyte" or "ms" for "millisecond") are converted to the appropriate
    /// enum variant, with other unknown units being mapped to [`Unit::Other`].
    #[inline]
    fn from(value: Cow<'static, str>) -> Self {
        Self::new(value)
    }
}

impl From<&'static str> for Unit {
    /// Convert a [`&'static str`](str) to a [`Unit`]. Known units (including standard symbols,
    /// such as "MB" for "megabyte" or "ms" for "millisecond") are converted to the appropriate
    /// enum variant, with other unknown units being mapped to [`Unit::Other`].
    #[inline]
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Unit {
    /// Convert a [`String`] to a [`Unit`]. Known units (including standard symbols,
    /// such as "MB" for "megabyte" or "ms" for "millisecond") are converted to the appropriate
    /// enum variant, with other unknown units being mapped to [`Unit::Other`].
    #[inline]
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for Unit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Unit::new(String::deserialize(deserializer)?))
    }
}

impl Unit {
    fn new<U>(value: U) -> Self
    where
        U: Into<Cow<'static, str>>,
    {
        let value = value.into();

        match value.as_ref() {
            "nanosecond" | "Nanosecond" | "nanoseconds" | "Nanoseconds" | "ns" => Self::Nanosecond,
            // Note: μs (with μ: U+03BC, GREEK SMALL LETTER MU) and µs (with µ: U+00B5, MICRO SIGN)
            // look nearly identical, both convert to micro.
            "microsecond" | "Microsecond" | "microseconds" | "Microseconds" | "μs" | "µs" => {
                Self::Microsecond
            }
            "millisecond" | "Millisecond" | "milliseconds" | "Milliseconds" | "ms" => {
                Self::Millisecond
            }
            "second" | "Second" | "seconds" | "Seconds" | "s" => Self::Second,
            "minute" | "Minute" | "minutes" | "Minutes" | "min" => Self::Minute,
            "hour" | "Hour" | "hours" | "Hours" | "h" => Self::Hour,
            "day" | "Day" | "days" | "Days" | "d" => Self::Day,
            "week" | "Week" | "weeks" | "Weeks" => Self::Week,
            "bit" | "Bit" | "bits" | "Bits" | "b" => Self::Bit,
            "byte" | "Byte" | "bytes" | "Bytes" | "B" => Self::Byte,
            "kilobyte" | "Kilobyte" | "kilobytes" | "Kilobytes" | "kB" => Self::Kilobyte,
            "kibibyte" | "Kibibyte" | "kibibytes" | "Kibibytes" | "KiB" => Self::Kibibyte,
            "megabyte" | "Megabyte" | "megabytes" | "Megabytes" | "MB" => Self::Megabyte,
            "mebibyte" | "Mebibyte" | "mebibytes" | "Mebibytes" | "MiB" => Self::Mebibyte,
            "gigabyte" | "Gigabyte" | "gigabytes" | "Gigabytes" | "GB" => Self::Gigabyte,
            "gibibyte" | "Gibibyte" | "gibibytes" | "Gibibytes" | "GiB" => Self::Gibibyte,
            "terabyte" | "Terabyte" | "terabytes" | "Terabytes" | "TB" => Self::Terabyte,
            "tebibyte" | "Tebibyte" | "tebibytes" | "Tebibytes" | "TiB" => Self::Tebibyte,
            "petabyte" | "Petabyte" | "petabytes" | "Petabytes" | "PB" => Self::Petabyte,
            "pebibyte" | "Pebibyte" | "pebibytes" | "Pebibytes" | "PiB" => Self::Pebibyte,
            "exabyte" | "Exabyte" | "exabytes" | "Exabytes" | "EB" => Self::Exabyte,
            "exbibyte" | "Exbibyte" | "exbibytes" | "Exbibytes" | "EiB" => Self::Exbibyte,
            "ratio" | "Ratio" => Self::Ratio,
            "percent" | "Percent" | "%" => Self::Percent,
            _ => Self::Other(value),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Test that "μs" (with U+03BC) resolves as microseconds
    #[test]
    fn greek_small_letter_mu_resolves_as_microseconds() {
        let unit = Unit::from("μs");
        assert_eq!(unit, Unit::Microsecond);
    }

    /// Test that µs (with U+00B5) also resolves as microseconds.
    #[test]
    fn micro_sign_resolves_as_microseconds() {
        let unit = Unit::from("µs");
        assert_eq!(unit, Unit::Microsecond);
    }
}
