use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use rand::random;

use crate::backtrace_support::process_event_stacktrace;
pub use crate::clientoptions::ClientOptions;
use crate::constants::SDK_INFO;
use crate::internals::{Dsn, Uuid};
use crate::protocol::{DebugMeta, Event};
use crate::scope::Scope;
use crate::transport::Transport;
use crate::utils;

impl<T: Into<ClientOptions>> From<T> for Client {
    fn from(o: T) -> Client {
        Client::with_options(o.into())
    }
}
/// The Sentry client object.
pub struct Client {
    options: ClientOptions,
    transport: RwLock<Option<Arc<dyn Transport>>>,
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

    /// Creates a new sentry client for the given options.
    ///
    /// If the DSN on the options is set to `None` the client will be entirely
    /// disabled.
    pub fn with_options(options: ClientOptions) -> Client {
        let create_transport = || {
            options.dsn.as_ref()?;
            let factory = options.transport.as_ref()?;
            Some(factory.create_transport(&options))
        };
        let transport = RwLock::new(create_transport());
        Client { options, transport }
    }

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
