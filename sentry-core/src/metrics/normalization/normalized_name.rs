use std::{borrow::Cow, sync::OnceLock};

use regex::Regex;

pub struct NormalizedName<'a> {
    name: Cow<'a, str>,
}

impl<'a> From<&'a str> for NormalizedName<'a> {
    fn from(name: &'a str) -> Self {
        static METRIC_NAME_RE: OnceLock<Regex> = OnceLock::new();
        Self {
            name: METRIC_NAME_RE
                .get_or_init(|| Regex::new(r"[^a-zA-Z0-9_\-.]").expect("Regex should compile"))
                .replace_all(super::truncate(name, 150), "_"),
        }
    }
}

impl std::fmt::Display for NormalizedName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[cfg(test)]
mod test {
    use crate::metrics::NormalizedName;

    #[test]
    fn test_from() {
        let expected = "aA1_-.____________";

        let actual = NormalizedName::from("aA1_-./+ö{😀\n\t\r\\| ,").to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_length_restriction() {
        let expected = "a".repeat(150);

        let actual = NormalizedName::from("a".repeat(155).as_ref()).to_string();

        assert_eq!(expected, actual);
    }
}
