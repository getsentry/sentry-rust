use std::fmt;

use backtrace::Backtrace;
use regex::{Captures, Regex};

use api::protocol::{Frame, Stacktrace};

lazy_static!{
    static ref HASH_FUNC_RE: Regex = Regex::new(r#"(?x)
        ^(.*)::h[a-f0-9]{16}$
    "#).unwrap();

    pub static ref WELL_KNOWN_SYS_MODULES: Vec<&'static str> = {
        #[allow(unused_mut)]
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
        #[allow(unused_mut)]
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
        #[cfg(feature = "with_error_chain")] {
            rv.push("error_chain::make_backtrace");
        }
        rv
    };
    pub static ref SECONDARY_BORDER_FRAMES: Vec<(&'static str, &'static str)> = {
        #![allow(unused_mut)]
        let mut rv = Vec::new();
        #[cfg(feature = "with_error_chain")] {
            rv.push(("error_chain::make_backtrace", "_<T as core..convert..Into<U>>::into"));
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

pub fn demangle_symbol(s: &str) -> String {
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
        }).to_string()
}

#[allow(unused)]
pub fn error_typename<D: fmt::Debug>(error: D) -> String {
    format!("{:?}", error)
        .split(&['(', '{'][..])
        .next()
        .unwrap()
        .trim()
        .into()
}

#[allow(unused)]
pub fn backtrace_to_stacktrace(bt: &Backtrace) -> Option<Stacktrace> {
    let frames = bt
        .frames()
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
                    instruction_addr: Some(frame.ip().into()),
                    abs_path,
                    filename,
                    lineno: sym.lineno().map(|l| l.into()),
                    colno: None,
                    ..Default::default()
                }
            })
        }).collect();
    Stacktrace::from_frames_reversed(frames)
}

/// Returns the current backtrace as sentry stacktrace.
#[allow(unused)]
pub fn current_stacktrace() -> Option<Stacktrace> {
    backtrace_to_stacktrace(&Backtrace::new())
}

/// A helper function to trim a stacktrace.
#[allow(unused)]
pub fn trim_stacktrace<F>(stacktrace: &mut Stacktrace, f: F)
where
    F: Fn(&Frame, &Stacktrace) -> bool,
{
    if let Some(cutoff) = stacktrace.frames.iter().rev().position(|frame| {
        if let Some(ref func) = frame.function {
            WELL_KNOWN_BORDER_FRAMES.contains(&func.as_str()) || f(frame, stacktrace)
        } else {
            false
        }
    }) {
        let secondary = {
            let func = stacktrace.frames[stacktrace.frames.len() - cutoff - 1]
                .function
                .as_ref()
                .unwrap();
            SECONDARY_BORDER_FRAMES
                .iter()
                .filter_map(|&(primary, secondary)| {
                    if primary == func {
                        Some(secondary)
                    } else {
                        None
                    }
                }).next()
        };
        let trunc = stacktrace.frames.len() - cutoff - 1;
        stacktrace.frames.truncate(trunc);

        if let Some(secondary) = secondary {
            if let Some(cutoff) = stacktrace.frames.iter().rev().position(|frame| {
                if let Some(ref func) = frame.function {
                    func.as_str() == secondary
                } else {
                    false
                }
            }) {
                let trunc = stacktrace.frames.len() - cutoff - 1;
                stacktrace.frames.truncate(trunc);
            }
        }
    }
}

/// Checks if a function is considered to be not in-app
#[allow(unused)]
pub fn is_sys_function(func: &str) -> bool {
    WELL_KNOWN_SYS_MODULES
        .iter()
        .any(|m| function_starts_with(func, m))
}

/// Checks whether the function name starts with the given pattern.
///
/// In trait implementations, the original type name is wrapped in "_< ... >" and colons are
/// replaced with dots. This function accounts for differences while checking.
pub fn function_starts_with(mut func_name: &str, pattern: &str) -> bool {
    if func_name.starts_with("_<") {
        func_name = &func_name[2..];
    }

    if !func_name.is_char_boundary(pattern.len()) {
        return false;
    }

    func_name
        .chars()
        .zip(pattern.chars())
        .all(|(f, p)| f == p || f == '.' && p == ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_starts_with() {
        assert!(function_starts_with(
            "futures::task_impl::std::set",
            "futures::"
        ));

        assert!(!function_starts_with(
            "futures::task_impl::std::set",
            "tokio::"
        ));
    }

    #[test]
    fn test_function_starts_with_impl() {
        assert!(function_starts_with(
            "_<futures..task_impl..Spawn<T>>::enter::_{{closure}}",
            "futures::"
        ));

        assert!(!function_starts_with(
            "_<futures..task_impl..Spawn<T>>::enter::_{{closure}}",
            "tokio::"
        ));
    }
}
