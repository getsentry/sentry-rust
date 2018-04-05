use std::fmt;

use regex::Regex;
use backtrace::Backtrace;

use api::protocol::{FileLocation, Frame, InstructionInfo, Stacktrace};

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
            "sentry::",
            "sentry_types::",
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

pub fn backtrace_to_stacktrace(bt: &Backtrace) -> Option<Stacktrace> {
    let frames = bt.frames()
        .iter()
        .flat_map(|frame| {
            frame.symbols().iter().map(move |sym| {
                let abs_path = sym.filename().map(|m| m.to_string_lossy().to_string());
                let filename = abs_path.as_ref().map(|p| filename(p));
                let symbol = sym.name().map_or("<unknown>".into(), |n| n.to_string());
                let function = sanitize_symbol(&symbol).to_string();
                Frame {
                    symbol: if symbol != function {
                        Some(symbol)
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
