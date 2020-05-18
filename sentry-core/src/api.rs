use crate::protocol::{Event, Level};
use crate::types::Uuid;
use crate::{Hub, IntoBreadcrumbs, Scope};

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
/// # use sentry_core as sentry;
/// use sentry::protocol::{Event, Level};
///
/// sentry::capture_event(Event {
///     message: Some("Hello World!".into()),
///     level: Level::Info,
///     ..Default::default()
/// });
/// ```
pub fn capture_event(event: Event<'static>) -> Uuid {
    Hub::with_active(|hub| hub.capture_event(event))
}

/// Captures an arbitrary message.
///
/// This creates an event from the given message and sends it to the current hub.
pub fn capture_message(msg: &str, level: Level) -> Uuid {
    Hub::with_active(|hub| hub.capture_message(msg, level))
}

/// Records a breadcrumb by calling a function.
///
/// The total number of breadcrumbs that can be recorded are limited by the
/// configuration on the client.  This function accepts any object that
/// implements `IntoBreadcrumbs` which is implemented for a varienty of
/// common types.  For efficiency reasons you can also pass a closure returning
/// a breadcrumb in which case the closure is only called if the client is
/// enabled.
///
/// The most common implementations that can be passed:
///
/// * `Breadcrumb`: to record a breadcrumb
/// * `Vec<Breadcrumb>`: to record more than one breadcrumb in one go.
/// * `Option<Breadcrumb>`: to record a breadcrumb or not
/// * additionally all of these can also be returned from an `FnOnce()`
///
/// # Example
///
/// ```
/// # use sentry_core as sentry;
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
pub fn add_breadcrumb<B: IntoBreadcrumbs>(breadcrumb: B) {
    Hub::with_active(|hub| hub.add_breadcrumb(breadcrumb))
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
/// ```
/// # use sentry_core as sentry;
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
    Hub::with_active(|hub| hub.configure_scope(f))
}

/// Temporarily pushes a scope for a single call optionally reconfiguring it.
///
/// This function takes two arguments: the first is a callback that is passed
/// a scope and can reconfigure it.  The second is callback that then executes
/// in the context of that scope.
///
/// This is useful when extra data should be send with a single capture call
/// for instance a different level or tags:
///
/// ```
/// # use sentry_core as sentry;
/// use sentry::{capture_message, with_scope, Level};
///
/// with_scope(
///     |scope| scope.set_level(Some(Level::Warning)),
///     || capture_message("some error", Level::Info),
/// );
/// ```
pub fn with_scope<C, F, R>(scope_config: C, callback: F) -> R
where
    C: FnOnce(&mut Scope),
    F: FnOnce() -> R,
{
    #[cfg(feature = "client")]
    {
        Hub::with(|hub| {
            if hub.is_active_and_usage_safe() {
                hub.with_scope(scope_config, callback)
            } else {
                callback()
            }
        })
    }
    #[cfg(not(feature = "client"))]
    {
        let _scope_config = scope_config;
        callback()
    }
}

/// Returns the last event ID captured.
///
/// This uses the current thread local hub.
pub fn last_event_id() -> Option<Uuid> {
    with_client_impl! {{
        Hub::with(|hub| hub.last_event_id())
    }}
}
