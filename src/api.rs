use std::env;
use std::sync::Arc;
use std::ffi::{OsStr, OsString};

use uuid::Uuid;

use api::protocol::{Breadcrumb, Event, Exception, Level};
use scope::{with_client_and_scope, with_stack};
use utils::current_stacktrace;

// public api from other crates
pub use sentry_types::{Dsn, ProjectId};
pub use sentry_types::protocol::v7 as protocol;

// public exports from this crate
pub use client::{Client, ClientOptions};
pub use scope::{pop_scope, push_scope};

/// Helper trait to convert an object into a client config
/// for create.
pub trait IntoClientConfig {
    /// Converts the object into a client config tuple of
    /// DSN and options.
    ///
    /// This can panic in cases where the conversion cannot be
    /// performed due to an error.
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>);
}

impl IntoClientConfig for () {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (None, None)
    }
}

impl<C: IntoClientConfig> IntoClientConfig for Option<C> {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        self.map(|x| x.into_client_config()).unwrap_or((None, None))
    }
}

impl<'a> IntoClientConfig for &'a str {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (Some(self.parse().unwrap()), None)
    }
}

impl<'a> IntoClientConfig for &'a OsStr {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (Some(self.to_string_lossy().parse().unwrap()), None)
    }
}

impl IntoClientConfig for OsString {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (Some(self.to_string_lossy().parse().unwrap()), None)
    }
}

impl IntoClientConfig for String {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (Some(self.parse().unwrap()), None)
    }
}

impl IntoClientConfig for Dsn {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        (Some(self), None)
    }
}

impl<C: IntoClientConfig> IntoClientConfig for (C, ClientOptions) {
    fn into_client_config(self) -> (Option<Dsn>, Option<ClientOptions>) {
        let (dsn, _) = self.0.into_client_config();
        (dsn, Some(self.1))
    }
}

/// Creates the sentry client for a given DSN and binds it.
pub fn init<C: IntoClientConfig>(cfg: C) {
    let (dsn, options) = cfg.into_client_config();
    let dsn = dsn.or_else(|| {
        env::var("SENTRY_DSN")
            .ok()
            .and_then(|dsn| dsn.parse::<Dsn>().ok())
    });
    if let Some(dsn) = dsn {
        let client = if let Some(options) = options {
            Client::with_options(dsn, options)
        } else {
            Client::new(dsn)
        };
        bind_client(Arc::new(client));
    }
}

/// Returns the currently bound client if there is one.
///
/// This might return `None` in case there is no client.  For the most part
/// code will not use this function but instead directly call `capture_event`
/// and similar functions which work on the currently active client.
pub fn current_client() -> Option<Arc<Client>> {
    with_stack(|stack| stack.client())
}

/// Rebinds the client on the current scope.
///
/// The current scope is defined as the current thread.  If a new thread spawns
/// it inherits the client of the process.  The main thread is specially handled
/// in the sense that if the main thread binds a client it becomes bound to the
/// process.
pub fn bind_client(client: Arc<Client>) {
    with_stack(|stack| stack.bind_client(client));
}

/// Captures an event on the currently active client if any.
///
/// The event must already be assembled.  Typically code would instead use
/// the utility methods like `capture_exception`.
pub fn capture_event(event: Event) -> Uuid {
    with_client_and_scope(|client, scope| client.capture_event(event, Some(scope)))
}

/// Captures an error.
///
/// This attaches the current stacktrace automatically.
pub fn capture_exception(ty: &str, value: Option<String>) -> Uuid {
    with_client_and_scope(|client, scope| {
        let event = Event {
            exceptions: vec![
                Exception {
                    ty: ty.to_string(),
                    value: value,
                    stacktrace: current_stacktrace(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        client.capture_event(event, Some(scope))
    })
}

/// Captures an arbitrary message.
pub fn capture_message(msg: &str, level: Level) -> Uuid {
    with_client_and_scope(|client, scope| {
        let event = Event {
            message: Some(msg.to_string()),
            level: level,
            ..Default::default()
        };
        client.capture_event(event, Some(scope))
    })
}

/// Records a breadcrumb by calling a function.
///
/// The total number of breadcrumbs that can be recorded are limited by the
/// configuration on the client.  This takes a callback because if the client
/// is not interested in breadcrumbs none will be recorded.
pub fn add_breadcrumb<F: FnOnce() -> Breadcrumb>(f: F) {
    with_client_and_scope(|client, scope| {
        scope.breadcrumbs.push_back(f());
        let limit = client.options().max_breadcrumbs;
        while scope.breadcrumbs.len() > limit {
            scope.breadcrumbs.pop_front();
        }
    })
}
