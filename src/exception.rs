use api::protocol;

/// Helper trait to return us backtrace and error info.
///
/// Various things can be converted into Sentry exceptions.  Currently
/// implemented are `failure::Error`, the `compat::ErrorChain` wrapper
/// to work with error chains and a few more.
pub trait Exception {
    /// Return a list of Sentry exception objects.
    fn exceptions(&self) -> Vec<protocol::Exception> {
        Vec::new()
    }

    /// Return the intended level of severity for the exception.
    fn level(&self) -> protocol::Level {
        protocol::Level::Error
    }
}

pub fn current_error_like() -> Box<Exception> {
    unreachable!()
}

lazy_static! {
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

#[cfg(any(feature = "with_error_chain", feature = "with_failure"))]
mod backtrace_support {
    use std::fmt;
    use regex::Regex;

    lazy_static!{
        static ref HASH_FUNC_RE: Regex = Regex::new(r#"(?x)
            ^(.*)::h[a-f0-9]{16}$
        "#).unwrap();
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
}

#[cfg(feature = "with_error_chain")]
mod error_chain_support {
    use std::fmt::{Debug, Display};
    use error_chain::ChainedError;

    use super::*;
    use super::backtrace_support::*;

    /// Wrapper to report error chains.
    pub struct ErrorChain<'a, T>(pub &'a T)
    where
        T: ChainedError,
        T::ErrorKind: Debug + Display;

    impl<'a, T> Exception for ErrorChain<'a, T>
    where
        T: ChainedError,
        T::ErrorKind: Debug + Display,
    {
        fn exceptions(&self) -> Vec<protocol::Exception> {
            let mut rv = vec![];
            let error = self.0;

            rv.push(protocol::Exception {
                ty: error_typename(error.kind()),
                value: Some(error.kind().to_string()),
                stacktrace: error.backtrace().and_then(|backtrace| {
                    let frames = backtrace
                        .frames()
                        .iter()
                        .flat_map(|frame| {
                            frame.symbols().iter().map(move |sym| {
                                let abs_path =
                                    sym.filename().map(|m| m.to_string_lossy().to_string());
                                let filename = abs_path.as_ref().map(|p| filename(p));
                                let symbol =
                                    sym.name().map_or("<unknown>".into(), |n| n.to_string());
                                let function = sanitize_symbol(&symbol).to_string();
                                protocol::Frame {
                                    symbol: if symbol != function {
                                        Some(symbol)
                                    } else {
                                        None
                                    },
                                    function: Some(function),
                                    instruction_info: protocol::InstructionInfo {
                                        instruction_addr: Some(frame.ip().into()),
                                        ..Default::default()
                                    },
                                    location: protocol::FileLocation {
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
                    protocol::Stacktrace::from_frames_reversed(frames)
                }),
                ..Default::default()
            });

            for error in error.iter().skip(1) {
                rv.push(protocol::Exception {
                    ty: error_typename(error),
                    value: Some(error.to_string()),
                    ..Default::default()
                })
            }

            rv
        }
    }
}

#[cfg(feature = "with_error_chain")]
pub use self::error_chain_support::ErrorChain;

#[cfg(feature = "with_failure")]
mod failure_support {
    use super::*;
    use super::backtrace_support::*;

    use regex::Regex;
    use failure;
    use failure::Fail;

    lazy_static! {
        static ref FRAME_RE: Regex = Regex::new(r#"(?xm)
            ^
                [\ ]*(?:\d+:)[\ ]*                  # leading frame number
                (?P<addr>0x[a-f0-9]+)               # addr
                [\ ]-[\ ]
                (?P<symbol>[^\r\n]+)
                (?:
                    \r?\n
                    [\ \t]+at[\ ]
                    (?P<path>[^\r\n]+?)
                    (?::(?P<lineno>\d+))?
                )?
            $
        "#).unwrap();
    }

    fn parse_stacktrace(bt: &str) -> Option<protocol::Stacktrace> {
        let frames = FRAME_RE.captures_iter(&bt).map(|captures| {
            let abs_path = captures.name("path").map(|m| m.as_str().to_string());
            let filename = abs_path.as_ref().map(|p| filename(p));
            let symbol = captures["symbol"].to_string();
            let function = sanitize_symbol(&symbol).to_string();
            protocol::Frame {
                symbol: if symbol != function {
                    Some(symbol)
                } else {
                    None
                },
                function: Some(function),
                instruction_info: protocol::InstructionInfo {
                    instruction_addr: Some(captures["addr"].parse().unwrap()),
                    ..Default::default()
                },
                location: protocol::FileLocation {
                    abs_path: abs_path,
                    filename: filename,
                    line: captures
                        .name("lineno")
                        .map(|x| x.as_str().parse::<u64>().unwrap()),
                    column: None,
                },
                ..Default::default()
            }
        }).collect();

        protocol::Stacktrace::from_frames_reversed(frames)
    }

    fn fail_to_exception(f: &Fail, bt: Option<&failure::Backtrace>) -> protocol::Exception {
        protocol::Exception {
            ty: error_typename(f),
            value: Some(f.to_string()),
            stacktrace: bt.map(|backtrace| backtrace.to_string())
                .and_then(|x| parse_stacktrace(&x)),
            ..Default::default()
        }
    }

    impl<'a> Exception for failure::Error {
        fn exceptions(&self) -> Vec<protocol::Exception> {
            let mut rv = vec![];
            for (idx, cause) in self.causes().enumerate() {
                let bt = match cause.backtrace() {
                    Some(bt) => Some(bt),
                    // TODO: not 0, but effectively -1
                    None if idx == 0 => Some(self.backtrace()),
                    None => None,
                };
                rv.push(fail_to_exception(cause, bt));
            }
            rv
        }
    }
}
