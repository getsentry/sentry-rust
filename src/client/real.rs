use std::borrow::Cow;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use uuid::Uuid;

use api::protocol::{DebugMeta, Event};
use api::Dsn;
use backtrace_support::{function_starts_with, is_sys_function};
use constants::{SDK_INFO, USER_AGENT};
use hub::Hub;
use scope::Scope;
use transport::Transport;
use utils::{debug_images, server_name, trim_stacktrace};

/// The Sentry client object.
///
/// ## Shim Behavior
///
/// This type is technically available in Shim mode but cannot be constructed.
/// It's passed to some callbacks but those callbacks will never be executed if
/// the shim is not configured so a lot of the implementations are irrelevant as
/// the code is effectively dead.
///
/// To see what types are available in shim only mode refer to
/// [the shim client docs](shim/struct.Client.html).
#[derive(Clone)]
pub struct Client {
    options: ClientOptions,
    transport: Option<Arc<Transport>>,
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
#[derive(Debug, Clone)]
pub struct ClientOptions {
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
    /// The user agent that should be reported.
    pub user_agent: Cow<'static, str>,
    /// The timeout on client drop for draining events.
    pub drop_drain_timeout: Option<Duration>,
}

impl Default for ClientOptions {
    fn default() -> ClientOptions {
        ClientOptions {
            in_app_include: vec![],
            in_app_exclude: vec![],
            extra_border_frames: vec![],
            max_breadcrumbs: 100,
            trim_backtraces: true,
            release: None,
            environment: Some(if cfg!(debug_assertions) {
                "debug".into()
            } else {
                "release".into()
            }),
            server_name: server_name().map(Cow::Owned),
            user_agent: Cow::Borrowed(&USER_AGENT),
            drop_drain_timeout: Some(Duration::from_secs(2)),
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

/// Helper trait to convert an object into a client config and/or client
/// for `init`.
pub trait IntoClient: Sized {
    /// Converts the object into a client config tuple of
    /// DSN and options.
    ///
    /// This can panic in cases where the conversion cannot be
    /// performed due to an error.
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>);

    /// Converts the object into a client right away.
    fn into_client(self) -> Option<Client> {
        let (dsn, options) = self.into_client_config();
        let dsn = dsn.or_else(|| {
            env::var("SENTRY_DSN")
                .ok()
                .and_then(|dsn| dsn.parse::<Dsn>().ok())
        });
        if let Some(dsn) = dsn {
            Some(if let Some(options) = options {
                Client::with_dsn_and_options(dsn, options)
            } else {
                Client::with_dsn(dsn)
            })
        } else {
            None
        }
    }
}

impl IntoClient for Client {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (self.dsn().map(|x| x.clone()), Some(self.options().clone()))
    }

    fn into_client(self) -> Option<Client> {
        Some(self)
    }
}

impl IntoClient for () {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (None, None)
    }
}

impl<C: IntoClient> IntoClient for Option<C> {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        self.map(|x| x.into_client_config()).unwrap_or((None, None))
    }
}

impl<'a> IntoClient for &'a str {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        if self.is_empty() {
            (None, None)
        } else {
            (Some(self.parse().unwrap()), None)
        }
    }
}

impl<'a> IntoClient for &'a OsStr {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        if self.is_empty() {
            (None, None)
        } else {
            (Some(self.to_string_lossy().parse().unwrap()), None)
        }
    }
}

impl IntoClient for OsString {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        if self.is_empty() {
            (None, None)
        } else {
            (Some(self.to_string_lossy().parse().unwrap()), None)
        }
    }
}

impl IntoClient for String {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        if self.is_empty() {
            (None, None)
        } else {
            (Some(self.parse().unwrap()), None)
        }
    }
}

impl<'a> IntoClient for &'a Dsn {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (Some(self.clone()), None)
    }
}

impl IntoClient for Dsn {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (Some(self), None)
    }
}

impl<C: IntoClient> IntoClient for (C, ClientOptions) {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        let (dsn, _) = self.0.into_client_config();
        (dsn, Some(self.1))
    }
}

impl Client {
    /// Creates a new Sentry client from a config helper.
    ///
    /// As the config helper can also disable the client this method might return
    /// `None` instead.  This is what `sentry::init` uses internally before binding
    /// the client.
    ///
    /// The client config can be of one of many formats as implemented by the
    /// `IntoClient` trait.  The most common form is to just supply a
    /// string with the DSN.
    ///
    /// # Supported Configs
    ///
    /// The following common values are supported for the client config:
    ///
    /// * `()`: pick up the default config from the environment only
    /// * `&str` / `String` / `&OsStr` / `String`: configure the client with the given DSN
    /// * `Dsn` / `&Dsn`: configure the client with a given DSN
    /// * `(C, options)`: configure the client from the given DSN and optional options.
    ///
    /// The tuple form lets you do things like `(Dsn, ClientOptions)` for instance.
    ///
    /// # Panics
    ///
    /// The `IntoClient` can panic for the forms where a DSN needs to be parsed.
    /// If you want to handle invalid DSNs you need to parse them manually by calling
    /// parse on it and handle the error.
    pub fn from_config<C: IntoClient>(cfg: C) -> Option<Client> {
        cfg.into_client()
    }

    /// Creates a new sentry client for the given DSN.
    pub fn with_dsn(dsn: Dsn) -> Client {
        Client::with_dsn_and_options(dsn, Default::default())
    }

    /// Creates a new sentry client for the given DSN.
    pub fn with_dsn_and_options(dsn: Dsn, options: ClientOptions) -> Client {
        let transport = Transport::new(dsn, options.user_agent.to_string());
        Client {
            options,
            transport: Some(Arc::new(transport)),
        }
    }

    /// Creates a new client that does not send anything.
    ///
    /// This is useful when general sentry handling is wanted but a client cannot be bound
    /// yet as the DSN might not be available yet.  In that case a disabled client can be
    /// bound and later replaced by another one.
    ///
    /// A disabled client can be detected by inspecting the DSN.  If the DSN is `None` then
    /// the client is disabled.
    pub fn disabled() -> Client {
        Client::disabled_with_options(Default::default())
    }

    /// Creates a new client that does not send anything with custom options.
    pub fn disabled_with_options(options: ClientOptions) -> Client {
        Client {
            options,
            transport: None,
        }
    }

    #[cfg_attr(feature = "cargo-clippy", allow(cyclomatic_complexity))]
    fn prepare_event(&self, event: &mut Event, scope: Option<&Scope>) {
        lazy_static! {
            static ref DEBUG_META: DebugMeta = DebugMeta {
                images: debug_images(),
                ..Default::default()
            };
        }

        if let Some(scope) = scope {
            scope.apply_to_event(event);
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
    }

    /// Returns the options of this client.
    pub fn options(&self) -> &ClientOptions {
        &self.options
    }

    /// Returns the DSN that constructed this client.
    ///
    /// If the client is in disabled mode this returns `None`.
    pub fn dsn(&self) -> Option<&Dsn> {
        self.transport.as_ref().map(|x| x.dsn())
    }

    /// Captures an event and sends it to sentry.
    pub fn capture_event(&self, mut event: Event<'static>, scope: Option<&Scope>) -> Uuid {
        if let Some(ref transport) = self.transport {
            self.prepare_event(&mut event, scope);
            transport.send_event(event)
        } else {
            Default::default()
        }
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
}

/// Helper struct that is returned from `init`.
///
/// When this is dropped events are drained with a 1 second timeout.
pub struct ClientInitGuard(Option<Arc<Client>>);

impl ClientInitGuard {
    /// Returns `true` if a client was created by initialization.
    pub fn is_enabled(&self) -> bool {
        self.0.is_some()
    }

    /// Returns the client created by `init`.
    pub fn client(&self) -> Option<Arc<Client>> {
        self.0.clone()
    }
}

impl Drop for ClientInitGuard {
    fn drop(&mut self) {
        if let Some(ref client) = self.0 {
            client.drain_events(client.options.drop_drain_timeout);
        }
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
/// but gives a simplified interface that transparently handles clients not
/// being created by the Dsn being empty.
#[cfg(feature = "with_client_implementation")]
pub fn init<C: IntoClient>(cfg: C) -> ClientInitGuard {
    ClientInitGuard(Client::from_config(cfg).map(|client| {
        let client = Arc::new(client);
        Hub::with(|hub| hub.bind_client(Some(client.clone())));
        client
    }))
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
