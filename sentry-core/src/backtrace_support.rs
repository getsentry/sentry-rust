use std::fmt;

use backtrace::Backtrace;
use regex::{Captures, Regex};

#[cfg(feature = "with_client_implementation")]
use crate::client::ClientOptions;
use crate::protocol::{Frame, Stacktrace};

lazy_static::lazy_static! {
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
        "___rust_",
    ];
    #[cfg(feature = "with_failure")] {
        rv.push("failure::");
    }
    #[cfg(feature = "with_log")] {
        rv.push("log::");
    }
    #[cfg(feature = "with_error_chain")] {
        rv.push("error_chain::");
    }
    rv
};

pub static ref WELL_KNOWN_BORDER_FRAMES: Vec<&'static str> = {
    #[allow(unused_mut)]
    let mut rv = vec![
        "std::panicking::begin_panic",
        "core::panicking::panic",
    ];
    #[cfg(feature = "with_failure")] {
        rv.push("failure::error_message::err_msg");
        rv.push("failure::backtrace::Backtrace::new");
        rv.push("failure::backtrace::internal::InternalBacktrace::new");
        rv.push("failure::Fail::context");
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

    static ref CRATE_RE: Regex = Regex::new(r#"(?x)
        ^
        (?:_?<)?           # trait impl syntax
        (?:\w+\ as \ )?    # anonymous implementor
        ([a-zA-Z0-9_]+?)   # crate name
        (?:\.\.|::)        # crate delimiter (.. or ::)
    "#).unwrap();
}

/// Tries to parse the rust crate from a function name.
#[cfg(any(test, feature = "with_client_implementation"))]
fn parse_crate_name(func_name: &str) -> Option<String> {
    CRATE_RE
        .captures(func_name)
        .and_then(|caps| caps.get(1))
        .map(|cr| cr.as_str().into())
}

#[cfg(feature = "with_client_implementation")]
pub fn process_event_stacktrace(stacktrace: &mut Stacktrace, options: &ClientOptions) {
    // automatically trim backtraces
    if options.trim_backtraces {
        trim_stacktrace(stacktrace, |frame, _| {
            if let Some(ref func) = frame.function {
                options.extra_border_frames.contains(&func.as_str())
            } else {
                false
            }
        })
    }

    // automatically prime in_app and set package
    let mut any_in_app = false;
    for frame in &mut stacktrace.frames {
        let func_name = match frame.function {
            Some(ref func) => func,
            None => continue,
        };

        // set package if missing to crate prefix
        if frame.package.is_none() {
            frame.package = parse_crate_name(func_name);
        }

        match frame.in_app {
            Some(true) => {
                any_in_app = true;
                continue;
            }
            Some(false) => {
                continue;
            }
            None => {}
        }

        for m in &options.in_app_exclude {
            if function_starts_with(func_name, m) {
                frame.in_app = Some(false);
                break;
            }
        }

        if frame.in_app.is_some() {
            continue;
        }

        for m in &options.in_app_include {
            if function_starts_with(func_name, m) {
                frame.in_app = Some(true);
                any_in_app = true;
                break;
            }
        }

        if frame.in_app.is_some() {
            continue;
        }

        if is_sys_function(func_name) {
            frame.in_app = Some(false);
        }
    }

    if !any_in_app {
        for frame in &mut stacktrace.frames {
            if frame.in_app.is_none() {
                frame.in_app = Some(true);
            }
        }
    }
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
        .replace_all(s, |caps: &Captures<'_>| {
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
        })
        .to_string()
}

#[allow(unused)]
pub fn error_typename<D: fmt::Debug>(error: D) -> String {
    format!("{:?}", error)
        .split(&[' ', '(', '{', '\r', '\n'][..])
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
                        lineno: sym.lineno().map(u64::from),
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
        func_name = func_name.trim_start_matches('<').trim_start_matches("_<");
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

    #[test]
    fn test_parse_crate_name() {
        assert_eq!(
            parse_crate_name("futures::task_impl::std::set"),
            Some("futures".into())
        );
    }

    #[test]
    fn test_parse_crate_name_impl() {
        assert_eq!(
            parse_crate_name("_<futures..task_impl..Spawn<T>>::enter::_{{closure}}"),
            Some("futures".into())
        );
    }

    #[test]
    fn test_parse_crate_name_anonymous_impl() {
        assert_eq!(
            parse_crate_name("_<F as alloc..boxed..FnBox<A>>::call_box"),
            Some("alloc".into())
        );
    }

    #[test]
    fn test_parse_crate_name_none() {
        assert_eq!(parse_crate_name("main"), None);
    }

    #[test]
    fn test_parse_crate_name_newstyle() {
        assert_eq!(
            parse_crate_name("<failure::error::Error as core::convert::From<F>>::from"),
            Some("failure".into())
        );
    }
}
