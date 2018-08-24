use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use rand::random;
use regex::Regex;
use uuid::Uuid;

use api::protocol::{DebugMeta, Event, RepoReference};
use api::Dsn;
use backtrace_support::{function_starts_with, is_sys_function, trim_stacktrace};
use constants::{SDK_INFO, USER_AGENT};
use hub::Hub;
use internals::DsnParseError;
use scope::Scope;
use transport::{DefaultTransportFactory, Transport, TransportFactory};
use utils::{debug_images, server_name};

/// The Sentry client object.
#[derive(Clone)]
pub struct Client {
    options: ClientOptions,
    transport: Option<Arc<Box<Transport>>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Client")
            .field("dsn", &self.dsn())
            .field("options", &self.options)
            .finish()
    }
}

/// Configuration settings for the client.
pub struct ClientOptions {
    /// The DSN to use.  If not set the client is effectively disabled.
    pub dsn: Option<Dsn>,
    /// The transport to use.
    ///
    /// This is typically either a boxed function taking the client options by
    /// reference and returning a `Transport`, a boxed `Arc<Transport>` or
    /// alternatively the `DefaultTransportFactory`.
    pub transport: Box<TransportFactory>,
    /// module prefixes that are always considered in_app
    pub in_app_include: Vec<&'static str>,
    /// module prefixes that are never in_app
    pub in_app_exclude: Vec<&'static str>,
    /// border frames which indicate a border from a backtrace to
    /// useless internals.  Some are automatically included.
    pub extra_border_frames: Vec<&'static str>,
    /// Maximum number of breadcrumbs (0 to disable feature).
    pub max_breadcrumbs: usize,
    /// Automatically trim backtraces of junk before sending.
    pub trim_backtraces: bool,
    /// The release to be sent with events.
    pub release: Option<Cow<'static, str>>,
    /// The repos to send along with the events.
    pub repos: HashMap<String, RepoReference>,
    /// The environment to be sent with events.
    pub environment: Option<Cow<'static, str>>,
    /// The server name to be reported.
    pub server_name: Option<Cow<'static, str>>,
    /// The sample rate for event submission (0.0 - 1.0, defaults to 1.0)
    pub sample_rate: f32,
    /// The user agent that should be reported.
    pub user_agent: Cow<'static, str>,
    /// An optional HTTP proxy to use.
    ///
    /// This will default to the `http_proxy` environment variable.
    pub http_proxy: Option<Cow<'static, str>>,
    /// An optional HTTPS proxy to use.
    ///
    /// This will default to the `HTTPS_PROXY` environment variable
    /// or `http_proxy` if that one exists.
    pub https_proxy: Option<Cow<'static, str>>,
    /// The timeout on client drop for draining events.
    pub shutdown_timeout: Option<Duration>,
    /// Attaches stacktraces to messages.
    pub attach_stacktrace: bool,
    /// If turned on some default PII informat is attached.
    pub send_default_pii: bool,
}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct TransportFactory;
        f.debug_struct("ClientOptions")
            .field("dsn", &self.dsn)
            .field("transport", &TransportFactory)
            .field("in_app_include", &self.in_app_include)
            .field("in_app_exclude", &self.in_app_exclude)
            .field("extra_border_frames", &self.extra_border_frames)
            .field("max_breadcrumbs", &self.max_breadcrumbs)
            .field("trim_backtraces", &self.trim_backtraces)
            .field("release", &self.release)
            .field("repos", &self.repos)
            .field("environment", &self.environment)
            .field("server_name", &self.server_name)
            .field("sample_rate", &self.sample_rate)
            .field("user_agent", &self.user_agent)
            .field("http_proxy", &self.http_proxy)
            .field("https_proxy", &self.https_proxy)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("attach_stacktrace", &self.attach_stacktrace)
            .field("send_default_pii", &self.send_default_pii)
            .finish()
    }
}

impl Clone for ClientOptions {
    fn clone(&self) -> ClientOptions {
        ClientOptions {
            dsn: self.dsn.clone(),
            transport: self.transport.clone_factory(),
            in_app_include: self.in_app_include.clone(),
            in_app_exclude: self.in_app_exclude.clone(),
            extra_border_frames: self.extra_border_frames.clone(),
            max_breadcrumbs: self.max_breadcrumbs,
            trim_backtraces: self.trim_backtraces,
            release: self.release.clone(),
            repos: self.repos.clone(),
            environment: self.environment.clone(),
            server_name: self.server_name.clone(),
            sample_rate: self.sample_rate,
            user_agent: self.user_agent.clone(),
            http_proxy: self.http_proxy.clone(),
            https_proxy: self.https_proxy.clone(),
            shutdown_timeout: self.shutdown_timeout,
            attach_stacktrace: self.attach_stacktrace,
            send_default_pii: self.send_default_pii,
        }
    }
}

impl Default for ClientOptions {
    fn default() -> ClientOptions {
        ClientOptions {
            // any invalid dsn including the empty string disables the dsn
            dsn: env::var("SENTRY_DSN")
                .ok()
                .and_then(|dsn| dsn.parse::<Dsn>().ok()),
            transport: Box::new(DefaultTransportFactory),
            in_app_include: vec![],
            in_app_exclude: vec![],
            extra_border_frames: vec![],
            max_breadcrumbs: 100,
            trim_backtraces: true,
            release: None,
            repos: Default::default(),
            environment: Some(if cfg!(debug_assertions) {
                "debug".into()
            } else {
                "release".into()
            }),
            server_name: server_name().map(Cow::Owned),
            sample_rate: 1.0,
            user_agent: Cow::Borrowed(&USER_AGENT),
            http_proxy: env::var("http_proxy").ok().map(Cow::Owned),
            https_proxy: env::var("https_proxy")
                .ok()
                .map(Cow::Owned)
                .or_else(|| env::var("HTTPS_PROXY").ok().map(Cow::Owned))
                .or_else(|| env::var("http_proxy").ok().map(Cow::Owned)),
            shutdown_timeout: Some(Duration::from_secs(2)),
            attach_stacktrace: false,
            send_default_pii: false,
        }
    }
}

lazy_static! {
    static ref CRATE_RE: Regex = Regex::new(r"^(?:_<)?([a-zA-Z0-9_]+?)(?:\.\.|::)").unwrap();
}

/// Tries to parse the rust crate from a function name.
fn parse_crate_name(func_name: &str) -> Option<String> {
    CRATE_RE
        .captures(func_name)
        .and_then(|caps| caps.get(1))
        .map(|cr| cr.as_str().into())
}

/// Helper trait to convert a string into an `Option<Dsn>`.
///
/// This converts a value into a DSN by parsing.  The empty string or
/// null values result in no DSN being parsed.
pub trait IntoDsn {
    /// Converts the value into a `Result<Option<Dsn>, E>`.
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError>;
}

impl<T: Into<ClientOptions>> From<T> for Client {
    fn from(o: T) -> Client {
        Client::with_options(o.into())
    }
}

impl<T: IntoDsn> From<(T, ClientOptions)> for ClientOptions {
    fn from((into_dsn, mut opts): (T, ClientOptions)) -> ClientOptions {
        opts.dsn = into_dsn.into_dsn().expect("invalid value for DSN");
        opts
    }
}

impl<T: IntoDsn> From<T> for ClientOptions {
    fn from(into_dsn: T) -> ClientOptions {
        ClientOptions {
            dsn: into_dsn.into_dsn().expect("invalid value for DSN"),
            ..ClientOptions::default()
        }
    }
}

impl<I: IntoDsn> IntoDsn for Option<I> {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        match self {
            Some(into_dsn) => into_dsn.into_dsn(),
            None => Ok(None),
        }
    }
}

impl IntoDsn for () {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        Ok(None)
    }
}

impl<'a> IntoDsn for &'a str {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        if self.is_empty() {
            Ok(None)
        } else {
            self.parse().map(Some)
        }
    }
}

impl<'a> IntoDsn for Cow<'a, str> {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        let x: &str = &self;
        x.into_dsn()
    }
}

impl<'a> IntoDsn for &'a OsStr {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        self.to_string_lossy().into_dsn()
    }
}

impl IntoDsn for OsString {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        self.as_os_str().into_dsn()
    }
}

impl IntoDsn for String {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        self.as_str().into_dsn()
    }
}

impl<'a> IntoDsn for &'a Dsn {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        Ok(Some(self.clone()))
    }
}

impl IntoDsn for Dsn {
    fn into_dsn(self) -> Result<Option<Dsn>, DsnParseError> {
        Ok(Some(self))
    }
}

impl Client {
    /// Creates a new Sentry client from a config.
    ///
    /// # Supported Configs
    ///
    /// The following common values are supported for the client config:
    ///
    /// * `ClientOptions`: configure the client with the given client options.
    /// * `()` or empty string: Disable the client.
    /// * `&str` / `String` / `&OsStr` / `String`: configure the client with the given DSN.
    /// * `Dsn` / `&Dsn`: configure the client with a given DSN.
    /// * `(Dsn, ClientOptions)`: configure the client from the given DSN and optional options.
    ///
    /// The `Default` implementation of `ClientOptions` pulls in the DSN from the
    /// `SENTRY_DSN` environment variable.
    ///
    /// # Panics
    ///
    /// The `Into<ClientOptions>` implementations can panic for the forms where a DSN needs to be
    /// parsed.  If you want to handle invalid DSNs you need to parse them manually by calling
    /// parse on it and handle the error.
    pub fn from_config<O: Into<ClientOptions>>(opts: O) -> Client {
        Client::with_options(opts.into())
    }

    #[doc(hidden)]
    #[deprecated(since = "0.8.0", note = "Please use Client::with_options instead")]
    pub fn with_dsn(dsn: Dsn) -> Client {
        let mut options = ClientOptions::default();
        options.dsn = Some(dsn);
        Client::with_options(options)
    }

    /// Creates a new sentry client for the given options.
    ///
    /// If the DSN on the options is set to `None` the client will be entirely
    /// disabled.
    pub fn with_options(options: ClientOptions) -> Client {
        #[cfg_attr(feature = "cargo-clippy", allow(question_mark))]
        let transport = if options.dsn.is_none() {
            None
        } else {
            Some(Arc::new(options.transport.create_transport(&options)))
        };
        Client { options, transport }
    }

    #[doc(hidden)]
    #[deprecated(since = "0.8.0", note = "Please use Client::with_options instead")]
    pub fn with_dsn_and_options(dsn: Dsn, mut options: ClientOptions) -> Client {
        options.dsn = Some(dsn);
        Client::with_options(options)
    }

    #[doc(hidden)]
    #[deprecated(since = "0.8.0", note = "Please use Client::with_options instead")]
    pub fn disabled() -> Client {
        Client::with_options(Default::default())
    }

    #[doc(hidden)]
    #[deprecated(since = "0.8.0", note = "Please use Client::with_options instead")]
    pub fn disabled_with_options(options: ClientOptions) -> Client {
        Client {
            options,
            transport: None,
        }
    }

    #[cfg_attr(feature = "cargo-clippy", allow(cyclomatic_complexity))]
    fn prepare_event(&self, event: &mut Event, scope: Option<&Scope>) -> Uuid {
        lazy_static! {
            static ref DEBUG_META: DebugMeta = DebugMeta {
                images: debug_images(),
                ..Default::default()
            };
        }

        if event.id.is_none() {
            event.id = Some(Uuid::new_v4());
        }

        if let Some(scope) = scope {
            scope.apply_to_event(event);
        }

        if event.release.is_none() {
            event.release = self.options.release.clone();
        }
        if event.repos.is_empty() && !self.options.repos.is_empty() {
            event.repos = self
                .options
                .repos
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
        }
        if event.environment.is_none() {
            event.environment = self.options.environment.clone();
        }
        if event.server_name.is_none() {
            event.server_name = self.options.server_name.clone();
        }
        if event.sdk_info.is_none() {
            event.sdk_info = Some(Cow::Borrowed(&SDK_INFO));
        }

        if &event.platform == "other" {
            event.platform = "native".into();
        }

        if event.debug_meta.is_empty() {
            event.debug_meta = Cow::Borrowed(&DEBUG_META);
        }

        for exc in &mut event.exceptions {
            if let Some(ref mut stacktrace) = exc.stacktrace {
                // automatically trim backtraces
                if self.options.trim_backtraces {
                    trim_stacktrace(stacktrace, |frame, _| {
                        if let Some(ref func) = frame.function {
                            self.options.extra_border_frames.contains(&func.as_str())
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

                    for m in &self.options.in_app_exclude {
                        if function_starts_with(func_name, m) {
                            frame.in_app = Some(false);
                            break;
                        }
                    }

                    if frame.in_app.is_some() {
                        continue;
                    }

                    for m in &self.options.in_app_include {
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
        }

        event.id.unwrap()
    }

    /// Returns the options of this client.
    pub fn options(&self) -> &ClientOptions {
        &self.options
    }

    /// Returns the DSN that constructed this client.
    ///
    /// If the client is in disabled mode this returns `None`.
    pub fn dsn(&self) -> Option<&Dsn> {
        self.options.dsn.as_ref()
    }

    /// Captures an event and sends it to sentry.
    pub fn capture_event(&self, mut event: Event<'static>, scope: Option<&Scope>) -> Uuid {
        if let Some(ref transport) = self.transport {
            if self.sample_should_send() {
                let event_id = self.prepare_event(&mut event, scope);
                transport.send_event(event);
                return event_id;
            }
        }
        Default::default()
    }

    /// Drains all pending events up to the current time.
    ///
    /// This returns `true` if the queue was successfully drained in the
    /// given time or `false` if not (for instance because of a timeout).
    /// If no timeout is provided the client will wait forever.
    pub fn drain_events(&self, timeout: Option<Duration>) -> bool {
        if let Some(ref transport) = self.transport {
            transport.drain(timeout)
        } else {
            true
        }
    }

    fn sample_should_send(&self) -> bool {
        let rate = self.options.sample_rate;
        if rate >= 1.0 {
            true
        } else {
            random::<f32>() <= rate
        }
    }
}

/// Helper struct that is returned from `init`.
///
/// When this is dropped events are drained with a 1 second timeout.
pub struct ClientInitGuard(Arc<Client>);

impl Drop for ClientInitGuard {
    fn drop(&mut self) {
        self.0.drain_events(self.0.options.shutdown_timeout);
    }
}

/// Creates the Sentry client for a given client config and binds it.
///
/// This returns a client init guard that if kept in scope will help the
/// client send events before the application closes by calling drain on
/// the generated client.  If the scope guard is immediately dropped then
/// no draining will take place so ensure it's bound to a variable.
///
/// # Examples
///
/// ```rust
/// fn main() {
///     let _sentry = sentry::init("https://key@sentry.io/1234");
/// }
/// ```
///
/// This behaves similar to creating a client by calling `Client::from_config`
/// and to then bind it to the hub except it's also possible to directly pass
/// a client.  For more information about the formats accepted see
/// `Client::from_config`.
#[cfg(feature = "with_client_implementation")]
pub fn init<C: Into<Client>>(cfg: C) -> ClientInitGuard {
    let client = Arc::new(cfg.into());
    Hub::with(|hub| hub.bind_client(Some(client.clone())));
    ClientInitGuard(client)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_parse_crate_name_unknown() {
        assert_eq!(
            parse_crate_name("_<F as alloc..boxed..FnBox<A>>::call_box"),
            None
        );
    }
}
