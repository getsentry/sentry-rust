use std::fmt;

use backtrace::Backtrace;
use regex::{Captures, Regex};

use api::protocol::{Frame, Stacktrace};

lazy_static! {
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
        "__rust_",
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
        rv.push("<sentry::integrations::log::Logger as log::Log>::log");
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
        rv.push(("error_chain::make_backtrace", "<T as core::convert::Into<U>>::into"));
    }
    {rv}
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
            }
            .to_string()
        })
        .to_string()
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
            // For each frame, there may be multiple symbols if a function was inlined, so
            // add an entry for each symbol.
            let symbols = frame.symbols();
            symbols
                .iter()
                .map(move |sym| {
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

                    // If there were no symbols at all, make sure to add at least one frame, as we
                    // may be able to symbolicate it on the server.
                })
                .chain(if symbols.is_empty() {
                    Some(Frame {
                        instruction_addr: Some(frame.ip().into()),
                        function: Some("<unknown>".into()),
                        ..Default::default()
                    })
                } else {
                    None
                })
        })
        .collect();
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
    let known_cutoff = stacktrace
        .frames
        .iter()
        .rev()
        .position(|frame| match frame.function {
            Some(ref func) => is_well_known(&func) || f(frame, stacktrace),
            None => false,
        });

    if let Some(cutoff) = known_cutoff {
        let secondary = {
            let func = stacktrace.frames[stacktrace.frames.len() - cutoff - 1]
                .function
                .as_ref()
                .unwrap();

            SECONDARY_BORDER_FRAMES
                .iter()
                .filter_map(|&(primary, secondary)| {
                    if function_starts_with(func, primary) {
                        Some(secondary)
                    } else {
                        None
                    }
                })
                .next()
        };
        let trunc = stacktrace.frames.len() - cutoff - 1;
        stacktrace.frames.truncate(trunc);

        if let Some(secondary) = secondary {
            let secondary_cutoff =
                stacktrace
                    .frames
                    .iter()
                    .rev()
                    .position(|frame| match frame.function {
                        Some(ref func) => function_starts_with(&func, secondary),
                        None => false,
                    });

            if let Some(cutoff) = secondary_cutoff {
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

/// Checks if a function is a well-known system function
fn is_well_known(func: &str) -> bool {
    WELL_KNOWN_BORDER_FRAMES
        .iter()
        .any(|m| function_starts_with(&func, m))
}

/// Checks whether the function name starts with the given pattern.
///
/// In trait implementations, the original type name is wrapped in "_< ... >" and colons are
/// replaced with dots. This function accounts for differences while checking.
pub fn function_starts_with(mut func_name: &str, mut pattern: &str) -> bool {
    if pattern.starts_with('<') {
        while pattern.starts_with('<') {
            pattern = &pattern[1..];

            if func_name.starts_with('<') {
                func_name = &func_name[1..];
            } else if func_name.starts_with("_<") {
                func_name = &func_name[2..];
            } else {
                return false;
            }
        }
    } else {
        func_name = func_name.trim_left_matches('<').trim_left_matches("_<");
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

    #[test]
    fn test_function_starts_with_newimpl() {
        assert!(function_starts_with(
            "<futures::task_impl::Spawn<T>>::enter::{{closure}}",
            "futures::"
        ));

        assert!(!function_starts_with(
            "<futures::task_impl::Spawn<T>>::enter::{{closure}}",
            "tokio::"
        ));
    }

    #[test]
    fn test_function_starts_with_impl_pattern() {
        assert!(function_starts_with(
            "_<futures..task_impl..Spawn<T>>::enter::_{{closure}}",
            "<futures::"
        ));

        assert!(function_starts_with(
            "<futures::task_impl::Spawn<T>>::enter::{{closure}}",
            "<futures::"
        ));

        assert!(!function_starts_with(
            "futures::task_impl::std::set",
            "<futures::"
        ));
    }
}
