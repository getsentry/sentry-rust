use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use rand::random;

use crate::backtrace_support::process_event_stacktrace;
pub use crate::clientoptions::ClientOptions;
use crate::constants::SDK_INFO;
use crate::hub::Hub;
use crate::internals::{Dsn, ParseDsnError, Uuid};
use crate::protocol::{DebugMeta, Event};
use crate::scope::Scope;
use crate::transport::Transport;
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

/// Helper trait to convert a string into an `Option<Dsn>`.
///
/// This converts a value into a DSN by parsing.  The empty string or
/// null values result in no DSN being parsed.
pub trait IntoDsn {
    /// Converts the value into a `Result<Option<Dsn>, E>`.
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError>;
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
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        match self {
            Some(into_dsn) => into_dsn.into_dsn(),
            None => Ok(None),
        }
    }
}

impl IntoDsn for () {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        Ok(None)
    }
}

impl<'a> IntoDsn for &'a str {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        if self.is_empty() {
            Ok(None)
        } else {
            self.parse().map(Some)
        }
    }
}

impl<'a> IntoDsn for Cow<'a, str> {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        let x: &str = &self;
        x.into_dsn()
    }
}

impl<'a> IntoDsn for &'a OsStr {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        self.to_string_lossy().into_dsn()
    }
}

impl IntoDsn for OsString {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        self.as_os_str().into_dsn()
    }
}

impl IntoDsn for String {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        self.as_str().into_dsn()
    }
}

impl<'a> IntoDsn for &'a Dsn {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        Ok(Some(self.clone()))
    }
}

impl IntoDsn for Dsn {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
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
                process_event_stacktrace(stacktrace, &self.options);
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
/// ```
/// # use sentry_core as sentry;
/// let _sentry = sentry::init("https://key@sentry.io/1234");
/// ```
///
/// Or if draining on shutdown should be ignored:
///
/// ```
/// # use sentry_core as sentry;
/// std::mem::forget(sentry::init("https://key@sentry.io/1234"));
/// ```
///
/// The guard returned can also be inspected to see if a client has been
/// created to enable further configuration:
///
/// ```
/// # use sentry_core as sentry;
/// use sentry::integrations::panic::register_panic_handler;
///
/// let sentry = sentry::init(sentry::ClientOptions {
///     release: Some("foo-bar-baz@1.0.0".into()),
///     ..Default::default()
/// });
/// if sentry.is_enabled() {
///     register_panic_handler();
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
