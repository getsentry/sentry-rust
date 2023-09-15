use std::fmt::{Debug, Display};
use std::ops::Deref;
use std::string::String as StdString;
use std::sync::Arc;

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct String(Arc<str>);

impl String {
    pub fn new() -> Self {
        Default::default()
    }
}

impl Default for String {
    fn default() -> Self {
        Self("".into())
    }
}

impl Display for String {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: &str = &self.0;
        write!(f, "{}", s)
    }
}

impl Debug for String {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: &str = &self.0;
        write!(f, "{:?}", s)
    }
}

impl AsRef<str> for String {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for String {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for String {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<StdString> for String {
    fn from(value: StdString) -> Self {
        Self(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_display() {
        assert_eq!(
            format!("{}", StdString::from("oh hi")),
            format!("{}", String::from("oh hi"))
        );
        assert_eq!(
            format!("{:?}", StdString::from("oh hi")),
            format!("{:?}", String::from("oh hi"))
        );
        println!("{:#?}", StdString::from("oh hi"));
        println!("{:#?}", String::from("oh hi"));
    }
}
