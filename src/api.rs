use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use api::protocol::{Breadcrumb, Event, Exception};
use scope::{with_client_and_scope, with_client_and_scope_mut, with_stack, with_stack_mut};
use utils::current_stacktrace;

// public api from other crates
pub use sentry_types::{Dsn, DsnParseError, ProjectId, ProjectIdParseError};
pub use sentry_types::protocol::v7 as protocol;
pub use sentry_types::protocol::v7::{Level, User};

// public exports from this crate
pub use client::{Client, ClientOptions, IntoClientConfig};
pub use scope::{push_scope, Scope, ScopeGuard};

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
            client.drain_events(Some(Duration::from_secs(2)));
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
pub fn init<C: IntoClientConfig>(cfg: C) -> ClientInitGuard {
    ClientInitGuard(Client::from_config(cfg).map(|client| {
        let client = Arc::new(client);
        bind_client(client.clone());
        client
    }))
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
    with_stack_mut(|stack| stack.bind_client(client));
}

/// Captures an event on the currently active client if any.
///
/// The event must already be assembled.  Typically code would instead use
/// the utility methods like `capture_exception`.  The return value is the
/// event ID.  In case Sentry is disabled the return value will be the nil
/// UUID (`Uuid::nil`).
///
/// # Example
///
/// ```
/// use sentry::protocol::{Event, Level};
///
/// sentry::capture_event(Event {
///     message: Some("Hello World!".into()),
///     level: Level::Info,
///     ..Default::default()
/// });
/// ```
pub fn capture_event(event: Event<'static>) -> Uuid {
    with_client_and_scope(|client, scope| client.capture_event(event, Some(scope)))
}

/// Captures an error.
///
/// This attaches the current stacktrace automatically.
///
/// # Example
///
/// ```
/// sentry::capture_exception("MyError", Some("This went horribly wrong".into()));
/// ```
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
///
/// # Example
///
/// ```
/// use sentry::protocol::{Breadcrumb, Map};
///
/// sentry::add_breadcrumb(|| Breadcrumb {
///     ty: "http".into(),
///     category: Some("request".into()),
///     data: {
///         let mut map = Map::new();
///         map.insert("method".into(), "GET".into());
///         map.insert("url".into(), "https://example.com/".into());
///         map
///     },
///     ..Default::default()
/// });
/// ```
pub fn add_breadcrumb<F: FnOnce() -> Breadcrumb>(f: F) {
    with_client_and_scope_mut(|client, scope| {
        let limit = client.options().max_breadcrumbs;
        if limit > 0 {
            scope.breadcrumbs = scope.breadcrumbs.push_back(f());
            while scope.breadcrumbs.len() > limit {
                if let Some((_, new)) = scope.breadcrumbs.pop_front() {
                    scope.breadcrumbs = new;
                }
            }
        }
    })
}

/// Drain events that are not yet sent of the current client.
///
/// This calls into `drain_events` of the currently active client.  See that function
/// for more information.
pub fn drain_events(timeout: Option<Duration>) {
    with_client_and_scope(|client, _| {
        client.drain_events(timeout);
    });
}

/// Invokes a function that can modify the current scope.
///
/// The function is passed a mutable reference to the `Scope` so that modifications
/// can be performed.  Because there might currently not be a scope or client active
/// it's possible that the callback might not be called at all.  As a result of this
/// the return value of this closure must have a default that is returned in such
/// cases.
///
/// # Example
///
/// ```rust
/// sentry::configure_scope(|scope| {
///     scope.set_user(Some(sentry::User {
///         username: Some("john_doe".into()),
///         ..Default::default()
///     }));
/// });
/// ```
///
/// # Panics
///
/// While the scope is being configured accessing scope related functionality is
/// not permitted.  In this case a wide range of panics will be raised.  It's
/// unsafe to call into `sentry::bind_client` or similar functions from within
/// the callback as a result of this.
pub fn configure_scope<F, R>(f: F) -> R
where
    R: Default,
    F: FnOnce(&mut Scope) -> R,
{
    if let Some((new_scope, rv)) = with_client_and_scope(|_, scope| {
        let mut new_scope = scope.clone();
        let rv = f(&mut new_scope);
        Some((new_scope, rv))
    }) {
        with_client_and_scope_mut(|_, scope| *scope = new_scope);
        rv
    } else {
        Default::default()
    }
}

/// A callback based alternative to using `push_scope`.
///
/// This that might look a bit nicer and is more consistent with some other
/// language integerations.
///
/// # Example
///
/// ```rust
/// # macro_rules! panic { ($e:expr) => {} }
/// sentry::with_scope(|| {
///     sentry::configure_scope(|scope| {
///         scope.set_user(Some(sentry::User {
///             username: Some("john_doe".into()),
///             ..Default::default()
///         }));
///     });
///     panic!("Something went wrong!");
/// });
/// ```
pub fn with_scope<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = push_scope();
    f()
}
