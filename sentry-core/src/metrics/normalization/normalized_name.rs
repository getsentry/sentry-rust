use regex::Regex;
use std::borrow::Cow;

pub struct NormalizedName<'a> {
    name: Cow<'a, str>,
}

impl<'a> From<&'a str> for NormalizedName<'a> {
    fn from(name: &'a str) -> Self {
        Self {
            name: Regex::new(r"[^a-zA-Z0-9_\-.]")
                .expect("Regex should compile")
                .replace_all(name, "_"),
        }
    }
}

impl std::fmt::Display for NormalizedName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
}
