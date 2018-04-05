use std::sync::Arc;

use uuid::Uuid;
use regex::Regex;

use api::Dsn;
use scope::Scope;
use protocol::Event;
use transport::Transport;
use backtrace_support::{WELL_KNOWN_BORDER_FRAMES, WELL_KNOWN_SYS_MODULES};

/// The Sentry client object.
#[derive(Debug, Clone)]
pub struct Client {
    dsn: Dsn,
    options: ClientOptions,
    transport: Arc<Transport>,
}

/// Configuration settings for the client.
#[derive(Debug, Clone)]
pub struct ClientOptions {
    /// module prefixes that are always considered in_app
    pub in_app_include: Vec<&'static str>,
    /// module prefixes that are never in_app
    pub in_app_exclude: Vec<&'static str>,
    /// border frames which indicate a border from a backtrace to
    /// useless internals.  Some are automatically included.
    pub extra_border_frames: Vec<&'static str>,
    /// Maximum number of breadcrumbs.
    pub max_breadcrumbs: usize,
}

impl Default for ClientOptions {
    fn default() -> ClientOptions {
        ClientOptions {
            in_app_include: vec![],
            in_app_exclude: vec![],
            extra_border_frames: vec![],
            max_breadcrumbs: 100,
        }
    }
}

lazy_static! {
    static ref CRATE_RE: Regex = Regex::new(r"^([^:]+?)::").unwrap();
}

impl Client {
    /// Creates a new sentry client for the given DSN.
    pub fn new(dsn: Dsn) -> Client {
        Client::with_options(dsn, Default::default())
    }

    /// Creates a new sentry client for the given DSN.
    pub fn with_options(dsn: Dsn, options: ClientOptions) -> Client {
        let transport = Transport::new(&dsn);
        Client {
            dsn: dsn,
            options: options,
            transport: Arc::new(transport),
        }
    }

    fn prepare_event(&self, event: &mut Event, scope: Option<&Scope>) {
        if let Some(scope) = scope {
            if !scope.breadcrumbs.is_empty() {
                event
                    .breadcrumbs
                    .extend(scope.breadcrumbs.iter().map(|x| x.clone()));
            }
        }

        if &event.platform == "other" {
            event.platform = "native".into();
        }

        for exc in event.exceptions.iter_mut() {
            if let Some(ref mut stacktrace) = exc.stacktrace {
                // automatically trim backtraces
                if let Some(cutoff) = stacktrace.frames.iter().rev().position(|frame| {
                    if let Some(ref func) = frame.function {
                        WELL_KNOWN_BORDER_FRAMES.contains(&func.as_str())
                            || self.options.extra_border_frames.contains(&func.as_str())
                    } else {
                        false
                    }
                }) {
                    let trunc = stacktrace.frames.len() - cutoff - 1;
                    stacktrace.frames.truncate(trunc);
                }

                // automatically prime in_app and set package
                let mut any_in_app = false;
                for frame in stacktrace.frames.iter_mut() {
                    let func_name = match frame.function {
                        Some(ref func) => func,
                        None => continue,
                    };

                    // set package if missing to crate prefix
                    if frame.package.is_none() {
                        frame.package = CRATE_RE
                            .captures(func_name)
                            .and_then(|caps| caps.get(1))
                            .map(|cr| cr.as_str().into());
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

                    for m in &self.options.in_app_exclude {
                        if func_name.starts_with(m) {
                            frame.in_app = Some(false);
                            break;
                        }
                    }

                    if frame.in_app.is_some() {
                        continue;
                    }

                    for m in &self.options.in_app_include {
                        if func_name.starts_with(m) {
                            frame.in_app = Some(true);
                            any_in_app = true;
                            break;
                        }
                    }

                    if frame.in_app.is_some() {
                        continue;
                    }

                    for m in WELL_KNOWN_SYS_MODULES.iter() {
                        if func_name.starts_with(m) {
                            frame.in_app = Some(false);
                            break;
                        }
                    }
                }

                if !any_in_app {
                    for frame in stacktrace.frames.iter_mut() {
                        if frame.in_app.is_none() {
                            frame.in_app = Some(true);
                        }
                    }
                }
            }
        }
    }

    /// Returns the options of this client.
    pub fn options(&self) -> &ClientOptions {
        &self.options
    }

    /// Returns the DSN that constructed this client.
    pub fn dsn(&self) -> &Dsn {
        &self.dsn
    }

    /// Captures an event and sends it to sentry.
    pub fn capture_event(&self, mut event: Event, scope: Option<&Scope>) -> Uuid {
        self.prepare_event(&mut event, scope);
        self.transport.send_event(event)
    }
}
