use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::panic::RefUnwindSafe;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use rand::random;
use regex::Regex;

use crate::backtrace_support::{function_starts_with, is_sys_function, trim_stacktrace};
use crate::constants::{SDK_INFO, USER_AGENT};
use crate::hub::Hub;
use crate::internals::{Dsn, DsnParseError, Uuid};
use crate::protocol::{Breadcrumb, DebugMeta, Event};
use crate::scope::Scope;
use crate::transport::{DefaultTransportFactory, Transport, TransportFactory};
use crate::utils;

/// The Sentry client object.
pub struct Client {
    options: ClientOptions,
    transport: RwLock<Option<Arc<Box<dyn Transport>>>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("dsn", &self.dsn())
            .field("options", &self.options)
            .finish()
    }
}

impl Clone for Client {
    fn clone(&self) -> Client {
        Client {
            options: self.options.clone(),
            transport: RwLock::new(self.transport.read().unwrap().clone()),
        }
    }
}

/// Type alias for before event/breadcrumb handlers.
pub type BeforeCallback<T> = Arc<Box<dyn Fn(T) -> Option<T> + Send + Sync>>;

/// Configuration settings for the client.
pub struct ClientOptions {
    /// The DSN to use.  If not set the client is effectively disabled.
    pub dsn: Option<Dsn>,
    /// The transport to use.
    ///
    /// This is typically either a boxed function taking the client options by
    /// reference and returning a `Transport`, a boxed `Arc<Transport>` or
    /// alternatively the `DefaultTransportFactory`.
    pub transport: Box<dyn TransportFactory>,
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
    /// The timeout on client drop for draining events on shutdown.
    pub shutdown_timeout: Duration,
    /// Enables debug mode.
    ///
    /// In debug mode debug information is printed to stderr to help you understand what
    /// sentry is doing.  When the `with_debug_to_log` flag is enabled Sentry will instead
    /// log to the `sentry` logger independently of this flag with the `Debug` level.
    pub debug: bool,
    /// Attaches stacktraces to messages.
    pub attach_stacktrace: bool,
    /// If turned on some default PII informat is attached.
    pub send_default_pii: bool,
    /// Before send callback.
    pub before_send: Option<BeforeCallback<Event<'static>>>,
    /// Before breadcrumb add callback.
    pub before_breadcrumb: Option<BeforeCallback<Breadcrumb>>,
}

// make this unwind safe.  It's not out of the box because of the contained `BeforeCallback`s.
impl RefUnwindSafe for ClientOptions {}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct TransportFactory;
        #[derive(Debug)]
        struct BeforeSendSet(bool);
        #[derive(Debug)]
        struct BeforeBreadcrumbSet(bool);
        f.debug_struct("ClientOptions")
            .field("dsn", &self.dsn)
            .field("transport", &TransportFactory)
            .field("in_app_include", &self.in_app_include)
            .field("in_app_exclude", &self.in_app_exclude)
            .field("extra_border_frames", &self.extra_border_frames)
            .field("max_breadcrumbs", &self.max_breadcrumbs)
            .field("trim_backtraces", &self.trim_backtraces)
            .field("release", &self.release)
            .field("environment", &self.environment)
            .field("server_name", &self.server_name)
            .field("sample_rate", &self.sample_rate)
            .field("user_agent", &self.user_agent)
            .field("http_proxy", &self.http_proxy)
            .field("https_proxy", &self.https_proxy)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("debug", &self.debug)
            .field("attach_stacktrace", &self.attach_stacktrace)
            .field("send_default_pii", &self.send_default_pii)
            .field("before_send", &BeforeSendSet(self.before_send.is_some()))
            .field(
                "before_send",
                &BeforeBreadcrumbSet(self.before_breadcrumb.is_some()),
            )
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
            environment: self.environment.clone(),
            server_name: self.server_name.clone(),
            sample_rate: self.sample_rate,
            user_agent: self.user_agent.clone(),
            http_proxy: self.http_proxy.clone(),
            https_proxy: self.https_proxy.clone(),
            shutdown_timeout: self.shutdown_timeout,
            debug: self.debug,
            attach_stacktrace: self.attach_stacktrace,
            send_default_pii: self.send_default_pii,
            before_send: self.before_send.clone(),
            before_breadcrumb: self.before_breadcrumb.clone(),
        }
    }
}

impl Default for ClientOptions {
    fn default() -> ClientOptions {
        ClientOptions {
            // any invalid dsn including the empty string disables the dsn
            dsn: std::env::var("SENTRY_DSN")
                .ok()
                .and_then(|dsn| dsn.parse::<Dsn>().ok()),
            transport: Box::new(DefaultTransportFactory),
            in_app_include: vec![],
            in_app_exclude: vec![],
            extra_border_frames: vec![],
            max_breadcrumbs: 100,
            trim_backtraces: true,
            release: std::env::var("SENTRY_RELEASE").ok().map(Cow::Owned),
            environment: std::env::var("SENTRY_ENVIRONMENT")
                .ok()
                .map(Cow::Owned)
                .or_else(|| {
                    Some(Cow::Borrowed(if cfg!(debug_assertions) {
                        "debug"
                    } else {
                        "release"
                    }))
                }),
            server_name: utils::server_name().map(Cow::Owned),
            sample_rate: 1.0,
            user_agent: Cow::Borrowed(&USER_AGENT),
            http_proxy: std::env::var("http_proxy").ok().map(Cow::Owned),
            https_proxy: std::env::var("https_proxy")
                .ok()
                .map(Cow::Owned)
                .or_else(|| std::env::var("HTTPS_PROXY").ok().map(Cow::Owned))
                .or_else(|| std::env::var("http_proxy").ok().map(Cow::Owned)),
            shutdown_timeout: Duration::from_secs(2),
            debug: false,
            attach_stacktrace: false,
            send_default_pii: false,
            before_send: None,
            before_breadcrumb: None,
        }
    }
}

lazy_static::lazy_static! {
    static ref CRATE_RE: Regex = Regex::new(r#"(?x)
        ^
        (?:_?<)?           # trait impl syntax
        (?:\w+\ as \ )?    # anonymous implementor
        ([a-zA-Z0-9_]+?)   # crate name
        (?:\.\.|::)        # crate delimiter (.. or ::)
    "#).unwrap();
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
        let transport = RwLock::new(match options.dsn {
            Some(_) => Some(Arc::new(options.transport.create_transport(&options))),
            None => None,
        });
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
            transport: RwLock::new(None),
        }
    }

    #[allow(clippy::cognitive_complexity)]
    fn prepare_event(
        &self,
        mut event: Event<'static>,
        scope: Option<&Scope>,
    ) -> Option<Event<'static>> {
        lazy_static::lazy_static! {
            static ref DEBUG_META: DebugMeta = DebugMeta {
                images: utils::debug_images(),
                ..Default::default()
            };
        }

        // id, debug meta and sdk are set before the processors run so that the
        // processors can poke around in that data.
        if event.event_id.is_nil() {
            event.event_id = Uuid::new_v4();
        }
        if event.debug_meta.is_empty() {
            event.debug_meta = Cow::Borrowed(&DEBUG_META);
        }
        if event.sdk.is_none() {
            event.sdk = Some(Cow::Borrowed(&SDK_INFO));
        }

        if let Some(scope) = scope {
            event = match scope.apply_to_event(event) {
                Some(event) => event,
                None => return None,
            };
        }

        if event.release.is_none() {
            event.release = self.options.release.clone();
        }
        if event.environment.is_none() {
            event.environment = self.options.environment.clone();
        }
        if event.server_name.is_none() {
            event.server_name = self.options.server_name.clone();
        }

        if &event.platform == "other" {
            event.platform = "native".into();
        }

        for exc in &mut event.exception {
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

        if let Some(ref func) = self.options.before_send {
            sentry_debug!("invoking before_send callback");
            let id = event.event_id;
            func(event).or_else(move || {
                sentry_debug!("before_send dropped event {:?}", id);
                None
            })
        } else {
            Some(event)
        }
    }

    /// Returns the options of this client.
    pub fn options(&self) -> &ClientOptions {
        &self.options
    }

    /// Returns the DSN that constructed this client.
    pub fn dsn(&self) -> Option<&Dsn> {
        self.options.dsn.as_ref()
    }

    /// Quick check to see if the client is enabled.
    pub fn is_enabled(&self) -> bool {
        self.options.dsn.is_some() && self.transport.read().unwrap().is_some()
    }

    /// Captures an event and sends it to sentry.
    pub fn capture_event(&self, event: Event<'static>, scope: Option<&Scope>) -> Uuid {
        if let Some(ref transport) = *self.transport.read().unwrap() {
            if self.sample_should_send() {
                if let Some(event) = self.prepare_event(event, scope) {
                    let event_id = event.event_id;
                    transport.send_event(event);
                    return event_id;
                }
            }
        }
        Default::default()
    }

    /// Drains all pending events and shuts down the transport behind the
    /// client.  After shutting down the transport is removed.
    ///
    /// This returns `true` if the queue was successfully drained in the
    /// given time or `false` if not (for instance because of a timeout).
    /// If no timeout is provided the client will wait for as long a
    /// `shutdown_timeout` in the client options.
    pub fn close(&self, timeout: Option<Duration>) -> bool {
        if let Some(transport) = self.transport.write().unwrap().take() {
            sentry_debug!("client close; request transport to shut down");
            transport.shutdown(timeout.unwrap_or(self.options.shutdown_timeout))
        } else {
            sentry_debug!("client close; no transport to shut down");
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
#[must_use = "when the init guard is dropped the transport will be shut down and no further \
              events can be sent.  If you do want to ignore this use mem::forget on it."]
pub struct ClientInitGuard(Arc<Client>);

impl ClientInitGuard {
    /// Quick check if the client is enabled.
    pub fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }
}

impl Drop for ClientInitGuard {
    fn drop(&mut self) {
        if self.is_enabled() {
            sentry_debug!("dropping client guard -> disposing client");
        } else {
            sentry_debug!("dropping client guard (no client to dispose)");
        }
        self.0.close(None);
    }
}

/// Creates the Sentry client for a given client config and binds it.
///
/// This returns a client init guard that must kept in scope will help the
/// client send events before the application closes.  When the guard is
/// dropped then the transport that was initialized shuts down and no
/// further events can be set on it.
///
/// If you don't want (or can) keep the guard around it's permissible to
/// call `mem::forget` on it.
///
/// # Examples
///
/// ```rust
/// fn main() {
///     let _sentry = sentry::init("https://key@sentry.io/1234");
/// }
/// ```
///
/// Or if draining on shutdown should be ignored:
///
/// ```rust
/// use std::mem;
///
/// fn main() {
///     mem::forget(sentry::init("https://key@sentry.io/1234"));
/// }
/// ```
///
/// The guard returned can also be inspected to see if a client has been
/// created to enable further configuration:
///
/// ```rust
/// use sentry::integrations::panic::register_panic_handler;
///
/// fn main() {
///     let sentry = sentry::init(sentry::ClientOptions {
///         release: Some("foo-bar-baz@1.0.0".into()),
///         ..Default::default()
///     });
///     if sentry.is_enabled() {
///         register_panic_handler();
///     }
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
    if let Some(dsn) = client.dsn() {
        sentry_debug!("enabled sentry client for DSN {}", dsn);
    } else {
        sentry_debug!("initialized disabled sentry client due to disabled or invalid DSN");
    }
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
