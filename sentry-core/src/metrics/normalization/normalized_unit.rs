use regex::Regex;

use crate::units::MetricUnit;

pub struct NormalizedUnit {
    unit: String,
}

impl From<MetricUnit> for NormalizedUnit {
    fn from(unit: MetricUnit) -> Self {
        let unsafe_unit = unit.to_string();
        let safe_unit = Regex::new(r"[^a-zA-Z0-9_]")
            .expect("Regex should compile")
            .replace_all(&unsafe_unit, "");
        let non_empty_safe_unit = match safe_unit.is_empty() {
            true => MetricUnit::None.to_string(),
            false => safe_unit.into(),
        };
        Self {
            unit: non_empty_safe_unit,
        }
    }
}

impl std::fmt::Display for NormalizedUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.unit)
    }
}

#[cfg(test)]
mod test {
    use crate::{metrics::NormalizedUnit, units::MetricUnit};

    #[test]
    fn test_from() {
        let unit = MetricUnit::Custom("aA1_-./+ö{😀\n\t\r\\| ,".into());
        let expected = "aA1_";

        let actual = NormalizedUnit::from(unit).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_from_empty() {
        let unit = MetricUnit::None;
        let expected = "none";

        let actual = NormalizedUnit::from(unit).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_from_empty_after_normalization() {
        let unit = MetricUnit::Custom("+".into());
        let expected = "none";

        let actual = NormalizedUnit::from(unit).to_string();

        assert_eq!(expected, actual);
    }
}
