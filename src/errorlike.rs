use api::protocol;

/// Helper trait to return us backtrace and error info.
pub trait ErrorLike {
    fn exceptions(&self) -> Vec<protocol::Exception> {
        Vec::new()
    }

    fn level(&self) -> protocol::Level {
        protocol::Level::Error
    }
}

pub fn current_error_like() -> Box<ErrorLike> {
    unreachable!()
}

#[cfg(feature = "with_failure")]
mod failure_support {
    use super::*;
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
        static ref HASH_FUNC_RE: Regex = Regex::new(r#"(?x)
            ^(.*)::h[a-f0-9]{16}$
        "#).unwrap();
    }

    fn pseudo_demangle(s: &str) -> &str {
        HASH_FUNC_RE
            .captures(s)
            .map(|c| c.get(1).unwrap().as_str())
            .unwrap_or(s)
    }

    fn parse_stacktrace(bt: &str) -> Option<protocol::Stacktrace> {
        let mut rv = vec![];

        for captures in FRAME_RE.captures_iter(&bt) {
            let abs_path = captures.name("path").map(|m| m.as_str().to_string());
            let filename = abs_path
                .as_ref()
                .map(|x| x.rsplitn(2, &['/', '\\'][..]).next().unwrap().to_string());
            let symbol = captures["symbol"].to_string();
            let function = pseudo_demangle(&symbol).to_string();
            rv.push(protocol::Frame {
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
            })
        }

        if rv.is_empty() {
            None
        } else {
            rv.reverse();
            Some(protocol::Stacktrace {
                frames: rv,
                ..Default::default()
            })
        }
    }

    fn fail_to_exception(f: &Fail, bt: Option<&failure::Backtrace>) -> protocol::Exception {
        protocol::Exception {
            ty: format!("{:?}", f)
                .split(&['{', '('][..])
                .next()
                .unwrap()
                .trim()
                .into(),
            value: Some(f.to_string()),
            stacktrace: bt.map(|backtrace| backtrace.to_string())
                .and_then(|x| parse_stacktrace(&x)),
            ..Default::default()
        }
    }

    impl<'a> ErrorLike for failure::Error {
        fn exceptions(&self) -> Vec<protocol::Exception> {
            let mut rv = vec![];
            for (idx, cause) in self.causes().enumerate() {
                let bt = match cause.backtrace() {
                    Some(bt) => Some(bt),
                    // TODO: not 0, but effectively -1
                    None if idx == 0 => Some(self.backtrace()),
                    None => None,
                };
                println!("{:?}", &bt);
                rv.push(fail_to_exception(cause, bt));
            }
            rv
        }
    }
}
