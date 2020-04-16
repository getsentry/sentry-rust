use crate::{Event, Hub, IntoBreadcrumbs, Level, Scope, Uuid};

/// Captures an event on the currently active client if any.
///
/// The event must already be assembled. Typically code would instead use
/// the utility methods like `capture_message` or other integration provided
/// methods.
///
/// The return value is the event ID, or `None` in case there is no active
/// client, or the event has been discarded due to rate limiting or one of the
/// event processors.
///
/// # Example
///
/// ```should_panic
/// use sentry_core::{Event, Level, Uuid};
///
/// let uuid = Uuid::new_v4();
/// let event = Event {
///     event_id: uuid,
///     message: Some("Hello World!".into()),
///     level: Level::Info,
///     ..Default::default()
/// };
///
/// assert_eq!(sentry_core::capture_event(event.clone()), None);
///
/// // TODO: init sentry
///
/// assert_eq!(sentry_core::capture_event(event), Some(uuid));
/// ```
pub fn capture_event(event: Event<'static>) -> Option<Uuid> {
    Hub::with_active(|hub| hub.capture_event(event))
}

/// Captures an arbitrary message.
///
/// This creates an event from the given message and sends it via
/// [`capture_event`](fn.capture_event.html).
pub fn capture_message(msg: &str, level: Level) -> Option<Uuid> {
    Hub::with_active(|hub| hub.capture_message(msg, level))
}

/// Records a new breadcrumb.
///
/// The total number of breadcrumbs that can be recorded is limited by the
/// configuration on the client. This function accepts any type that
/// implements `IntoBreadcrumbs` which is implemented for a varienty of
/// common types. For efficiency reasons you can also pass a closure returning
/// a breadcrumb in which case the closure is only called if the client is
/// enabled.
///
/// The most common implementations that can be passed:
///
/// * `Breadcrumb`: to directly record a single breadcrumb.
/// * `Vec<Breadcrumb>`: to record more than one breadcrumb in one go.
/// * `Option<Breadcrumb>`: to record an optional breadcrumb.
/// * additionally all of these can also be returned from a `FnOnce()`.
///
/// # Example
///
/// ```should_panic
/// use sentry_core::protocol::{Breadcrumb, Map};
///
/// sentry_core::add_breadcrumb(|| Breadcrumb {
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
///
/// // TODO: init, capture and assert breadcrumb
/// ```
pub fn add_breadcrumb<B: IntoBreadcrumbs>(breadcrumbs: B) {
    Hub::with_active(|hub| hub.add_breadcrumb(breadcrumbs))
}

/// Invokes a function that can modify the current scope.
///
/// The function is passed a mutable reference to the `Scope` so that
/// modifications can be performed. Because there might currently not be a
/// scope or client active it's possible that the callback might not be called
/// at all. As a result of this, the return value of this closure must have a
/// default that is returned in such cases.
///
/// # Example
///
/// ```compile_fail
/// sentry_core::configure_scope(|scope| {
///     scope.set_user(Some(sentry_core::User {
///         username: Some("john_doe".into()),
///         ..Default::default()
///     }));
/// });
///
/// // TODO: init, capture and assert user
/// ```
///
/// # Panics
///
/// TODO: update the comment
/// While the scope is being configured accessing scope related functionality is
/// not permitted. In this case a wide range of panics will be raised. It's
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
/// a scope and can reconfigure it. The second is a callback that then executes
/// in the context of that scope.
///
/// This is useful when extra data should be send with a single capture call,
/// for instance a different level or tags:
///
/// # Example
///
/// ```compile_fail
/// use sentry_core::Level;
///
/// sentry_core::with_scope(
///     |scope| scope.set_level(Level::Warning),
///     || sentry_core::capture_message("Foobar", Level::Info),
/// );
///
/// // TODO: init and assert the level override
/// ```
pub fn with_scope<C, F, R>(scope_config: C, callback: F) -> R
where
    C: FnOnce(&mut Scope),
    F: FnOnce() -> R,
{
    Hub::with(|hub| hub.with_scope(scope_config, callback))
}

/// Returns the last event ID captured.
///
/// Returns `None` if no even was previously captured, or all captured events
/// have been discarded.
///
/// # Example
///
/// ```should_panic
/// assert_eq!(sentry_core::last_event_id(), None);
///
/// // TODO: init, capture and assert the ID
/// ```
pub fn last_event_id() -> Option<Uuid> {
    Hub::with(|hub| hub.last_event_id())
}
