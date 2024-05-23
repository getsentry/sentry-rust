use itertools::Itertools;
use regex::Regex;
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;

use crate::metrics::TagMap;

pub struct NormalizedTags {
    tags: HashMap<String, String>,
}

impl From<TagMap> for NormalizedTags {
    fn from(tags: TagMap) -> Self {
        Self {
            tags: tags
                .iter()
                .map(|(k, v)| (Self::normalize_key(k), Self::normalize_value(v)))
                .filter(|(k, v)| !v.is_empty() && !k.is_empty())
                .collect(),
        }
    }
}

impl NormalizedTags {
    pub fn with_default_tags(mut self, tags: &TagMap) -> Self {
        tags.iter().for_each(|(k, v)| {
            self.tags
                .entry(Self::normalize_key(k))
                .or_insert(Self::normalize_value(v));
        });
        self
    }

    fn normalize_key(key: &str) -> String {
        Regex::new(r"[^a-zA-Z0-9_\-./]")
            .expect("Tag normalization regex should compile")
            .replace_all(&key.graphemes(true).take(32).collect::<String>(), "")
            .to_string()
    }

    fn normalize_value(value: &str) -> String {
        value
            .graphemes(true)
            .take(200)
            .collect::<String>()
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace('|', "\\u{7c}")
            .replace(',', "\\u{2c}")
    }
}

impl std::fmt::Display for NormalizedTags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let res = self
            .tags
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .sorted()
            .join(",");
        write!(f, "{res}")
    }
}

#[cfg(test)]
mod test {
    use super::NormalizedTags;
    use super::TagMap;

    #[test]
    fn test_replacement_characters() {
        let tags = TagMap::from_iter(
            [
                ("a\na", "a\na"),
                ("b\rb", "b\rb"),
                ("c\tc", "c\tc"),
                ("d\\d", "d\\d"),
                ("e|e", "e|e"),
                ("f,f", "f,f"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into())),
        );
        let expected = "aa:a\\na,bb:b\\rb,cc:c\\tc,dd:d\\\\d,ee:e\\u{7c}e,ff:f\\u{2c}f";

        let actual = NormalizedTags::from(tags).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_empty_tags() {
        let tags = TagMap::from_iter(
            [("+", "a"), ("a", ""), ("", "a"), ("", "")]
                .into_iter()
                .map(|(k, v)| (k.into(), v.into())),
        );
        let expected = "";

        let actual = NormalizedTags::from(tags).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_special_characters() {
        let tags = TagMap::from([("aA1_-./+ö{ 😀".into(), "aA1_-./+ö{ 😀".into())]);
        let expected = "aA1_-./:aA1_-./+ö{ 😀";

        let actual = NormalizedTags::from(tags).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_add_default_tags() {
        let default_tags = TagMap::from_iter(
            [
                ("release", "default_release"),
                ("environment", "production"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into())),
        );
        let expected = "environment:production,release:default_release";

        let actual = NormalizedTags::from(TagMap::new())
            .with_default_tags(&default_tags)
            .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_override_default_tags() {
        let default_tags = TagMap::from_iter(
            [
                ("release", "default_release"),
                ("environment", "production"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into())),
        );
        let expected = "environment:custom_env,release:custom_release";

        let actual = NormalizedTags::from(TagMap::from_iter(
            [("release", "custom_release"), ("environment", "custom_env")]
                .into_iter()
                .map(|(k, v)| (k.into(), v.into())),
        ))
        .with_default_tags(&default_tags)
        .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_tag_lengths() {
        let expected = "abcdefghijklmnopqrstuvwxyzabcde:abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopq🙂";

        let actual = NormalizedTags::from(TagMap::from([
            ("abcdefghijklmnopqrstuvwxyzabcde🙂fghijklmnopqrstuvwxyz".into(), 
            "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopq🙂rstuvwxyz".into()),
        ]))
        .to_string();

        assert_eq!(expected, actual);
    }
}
