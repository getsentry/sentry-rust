use std::{borrow::Cow, sync::OnceLock};

use regex::Regex;

use crate::units::MetricUnit;

pub struct NormalizedUnit<'a> {
    unit: Cow<'a, str>,
}

impl<'a> From<&'a str> for NormalizedUnit<'a> {
    fn from(unit: &'a str) -> Self {
        static METRIC_UNIT_RE: OnceLock<Regex> = OnceLock::new();
        let normalized_unit = METRIC_UNIT_RE
            .get_or_init(|| Regex::new(r"[^a-zA-Z0-9_]").expect("Regex should compile"))
            .replace_all(super::truncate(unit, 15), "");
        Self {
            unit: match normalized_unit.is_empty() {
                true => MetricUnit::None.to_string().into(),
                false => normalized_unit,
            },
        }
    }
}

impl std::fmt::Display for NormalizedUnit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.unit)
    }
}

#[cfg(test)]
mod test {
    use crate::metrics::NormalizedUnit;

    #[test]
    fn test_from() {
        let expected = "aA1_";

        let actual = NormalizedUnit::from("aA1_-./+ö{😀\n\t\r\\| ,").to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_from_empty() {
        let expected = "none";

        let actual = NormalizedUnit::from("").to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_from_empty_after_normalization() {
        let expected = "none";

        let actual = NormalizedUnit::from("+").to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_length_restriction() {
        let expected = "a".repeat(15);

        let actual = NormalizedUnit::from("a".repeat(20).as_ref()).to_string();

        assert_eq!(expected, actual);
    }
}
