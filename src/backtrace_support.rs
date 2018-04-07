use std::fmt;

use regex::{Captures, Regex};
use backtrace::Backtrace;

use api::protocol::{FileLocation, Frame, InstructionInfo, Stacktrace};

lazy_static!{
    static ref HASH_FUNC_RE: Regex = Regex::new(r#"(?x)
        ^(.*)::h[a-f0-9]{16}$
    "#).unwrap();

    pub static ref WELL_KNOWN_SYS_MODULES: Vec<&'static str> = {
        let mut rv = vec![
            "std::",
            "core::",
            "alloc::",
            "backtrace::",
            "sentry::",
            "sentry_types::",
            // these are not modules but things like __rust_maybe_catch_panic
            // or _<T as core..convert..Into<U>>::into
            "__rust_",
            "_<",
        ];
        #[cfg(feature = "with_failure")] {
            rv.push("failure::");
        }
        rv
    };
    pub static ref WELL_KNOWN_BORDER_FRAMES: Vec<&'static str> = {
        let mut rv = vec![
            "std::panicking::begin_panic",
        ];
        #[cfg(feature = "with_failure")] {
            rv.push("failure::error_message::err_msg");
            rv.push("failure::backtrace::Backtrace::new");
        }
        #[cfg(feature = "with_log")] {
            rv.push("_<sentry..integrations..log..Logger as log..Log>::log");
        }
        rv
    };

    pub static ref COMMON_RUST_SYMBOL_ESCAPES_RE: Regex = Regex::new(r#"(?x)
        \$
            (SP|BP|RF|LT|GT|LP|RP|C|
             u7e|u20|u27|u5b|u5d|u7b|u7d|u3b|u2b|u22)
        \$
    "#).unwrap();
}

pub fn filename(s: &str) -> String {
    s.rsplitn(2, &['/', '\\'][..]).next().unwrap().to_string()
}

pub fn strip_symbol(s: &str) -> &str {
    HASH_FUNC_RE
        .captures(s)
        .map(|c| c.get(1).unwrap().as_str())
        .unwrap_or(s)
}

pub fn demangle_symbol<'a>(s: &'a str) -> String {
    COMMON_RUST_SYMBOL_ESCAPES_RE
        .replace_all(s, |caps: &Captures| {
            match &caps[1] {
                "SP" => "@",
                "BP" => "*",
                "RF" => "&",
                "LT" => "<",
                "GT" => ">",
                "LP" => "(",
                "RP" => ")",
                "C" => ",",
                "u7e" => "~",
                "u20" => " ",
                "u27" => "'",
                "u5b" => "[",
                "u5d" => "]",
                "u7b" => "{",
                "u7d" => "}",
                "u3b" => ";",
                "u2b" => "+",
                "u22" => "\"",
                _ => unreachable!(),
            }.to_string()
        })
        .to_string()
}

pub fn error_typename<D: fmt::Debug>(error: D) -> String {
    format!("{:?}", error)
        .split(&['(', '{'][..])
        .next()
        .unwrap()
        .trim()
        .into()
}

pub fn backtrace_to_stacktrace(bt: &Backtrace) -> Option<Stacktrace> {
    let frames = bt.frames()
        .iter()
        .flat_map(|frame| {
            frame.symbols().iter().map(move |sym| {
                let abs_path = sym.filename().map(|m| m.to_string_lossy().to_string());
                let filename = abs_path.as_ref().map(|p| filename(p));
                let real_symbol = sym.name().map_or("<unknown>".into(), |n| n.to_string());
                let symbol = strip_symbol(&real_symbol);
                let function = demangle_symbol(symbol);
                Frame {
                    symbol: if symbol != function {
                        Some(symbol.into())
                    } else {
                        None
                    },
                    function: Some(function),
                    instruction_info: InstructionInfo {
                        instruction_addr: Some(frame.ip().into()),
                        ..Default::default()
                    },
                    location: FileLocation {
                        abs_path: abs_path,
                        filename: filename,
                        line: sym.lineno().map(|l| l as u64),
                        column: None,
                    },
                    ..Default::default()
                }
            })
        })
        .collect();
    Stacktrace::from_frames_reversed(frames)
}
