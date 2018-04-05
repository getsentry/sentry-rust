use std::fmt;
use regex::Regex;

lazy_static!{
    static ref HASH_FUNC_RE: Regex = Regex::new(r#"(?x)
        ^(.*)::h[a-f0-9]{16}$
    "#).unwrap();

    pub static ref WELL_KNOWN_SYS_MODULES: Vec<&'static str> = {
        let mut rv = vec![
            "__rust_",
            "std::",
            "core::",
            "alloc::",
            "backtrace::",
        ];
        #[cfg(feature = "with_failure")] {
            rv.push("failure::");
        }
        rv
    };
    pub static ref WELL_KNOWN_BORDER_FRAMES: Vec<&'static str> = {
        let mut rv = vec![];
        #[cfg(feature = "with_failure")] {
            rv.push("failure::error_message::err_msg");
        }
        rv
    };
}

pub fn filename(s: &str) -> String {
    s.rsplitn(2, &['/', '\\'][..]).next().unwrap().to_string()
}

pub fn sanitize_symbol(s: &str) -> &str {
    HASH_FUNC_RE
        .captures(s)
        .map(|c| c.get(1).unwrap().as_str())
        .unwrap_or(s)
}

pub fn error_typename<D: fmt::Debug>(error: D) -> String {
    format!("{:?}", error)
        .split(&['(', '{'][..])
        .next()
        .unwrap()
        .trim()
        .into()
}
