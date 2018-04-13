use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use api::protocol::{Breadcrumb, Event};

// public api from other crates
pub use sentry_types::{Dsn, DsnParseError, ProjectId, ProjectIdParseError};
pub use sentry_types::protocol::v7 as protocol;
pub use sentry_types::protocol::v7::{Level, User};

// public exports from this crate
pub use client::Client;
#[cfg(feature = "with_client_implementation")]
pub use client::{init, ClientInitGuard, ClientOptions, IntoClientConfig};
pub use scope::{bind_client, current_client, push_scope, with_client_and_scope, Scope, ScopeGuard};

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
#[allow(unused_variables)]
pub fn capture_event(event: Event<'static>) -> Uuid {
    with_client_impl! {{
        with_client_and_scope(|client, scope| client.capture_event(event, Some(scope)))
    }}
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
#[allow(unused_variables)]
pub fn capture_exception(ty: &str, value: Option<String>) -> Uuid {
    with_client_impl! {{
        use api::protocol::Exception;
        use utils::current_stacktrace;

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
    }}
}

/// Captures an arbitrary message.
#[allow(unused_variables)]
pub fn capture_message(msg: &str, level: Level) -> Uuid {
    with_client_impl! {{
        with_client_and_scope(|client, scope| {
            let event = Event {
                message: Some(msg.to_string()),
                level: level,
                ..Default::default()
            };
            client.capture_event(event, Some(scope))
        })
    }}
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
#[allow(unused_variables)]
pub fn add_breadcrumb<F: FnOnce() -> Breadcrumb>(f: F) {
    with_client_impl! {{
        use scope::with_client_and_scope_mut;
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
    }}
}

/// Drain events that are not yet sent of the current client.
///
/// This calls into `drain_events` of the currently active client.  See that function
/// for more information.
#[allow(unused_variables)]
pub fn drain_events(timeout: Option<Duration>) {
    with_client_impl! {{
        with_client_and_scope(|client, _| {
            client.drain_events(timeout);
        });
    }}
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
#[allow(unused_variables)]
pub fn configure_scope<F, R>(f: F) -> R
where
    R: Default,
    F: FnOnce(&mut Scope) -> R,
{
    with_client_impl! {{
        use scope::with_client_and_scope_mut;
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
    }}
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
#[allow(unused_variables)]
pub fn with_scope<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = push_scope();
    f()
}

/// A handle to the current scope.
///
/// A scope handle is returned by the `sentry::scope_handle` function and can be used to
/// transfer a scope to another thread.  The handle can be cloned to be used in multiple
/// threads.
///
/// A scope handle also implements `Default` which returns a dummy scope handle that does
/// not do anything on bind.
#[derive(Default, Clone)]
pub struct ScopeHandle(Option<(Arc<Client>, Arc<Scope>)>);

/// Returns the handle to the current scope.
///
/// This can be used to propagate a scope to another thread easily.  The parent thread
/// retrieves a handle and the child thread binds it.  A handle can be cloned so that
/// it can be used in multiple threads.
///
/// # Example
///
/// ```
/// use std::thread;
///
/// sentry::configure_scope(|scope| {
///     scope.set_tag("task", "task-name");
/// });
/// let handle = sentry::scope_handle();
/// thread::spawn(move || {
///     handle.bind();
///     // ...
/// });
/// ```
pub fn scope_handle() -> ScopeHandle {
    with_client_impl! {{
        with_client_and_scope(|client, scope| {
            ScopeHandle(Some((client, Arc::new(scope.clone()))))
        })
    }}
}

impl ScopeHandle {
    /// Binds the scope behind the handle to the current scope.
    pub fn bind(self) {
        with_client_impl! {{
            use scope::with_client_and_scope_mut;
            if let Some((src_client, src_scope)) = self.0 {
                bind_client(src_client);
                with_client_and_scope_mut(|_, scope| {
                    *scope = Arc::try_unwrap(src_scope)
                        .unwrap_or_else(|arc| (*arc).clone());
                })
            }
        }}
    }
}
