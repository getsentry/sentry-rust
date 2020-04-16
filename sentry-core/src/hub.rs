use crate::{Client, Event, IntoBreadcrumbs, Level, Scope, ScopeGuard, Uuid};

/// The central object that can manages scopes and clients.
///
/// This can be used to capture events and manage the scope. This object is
/// internally synchronized so it can be used from multiple threads if needed.
/// The default hub that is available automatically is thread local.
///
/// See the
/// [Unified API](https://docs.sentry.io/development/sdk-dev/unified-api/#hub)
/// documentation for further details.
///
/// In most situations developers do not need to interface with the Hub
/// directly. Instead, toplevel convenience functions are exposed that will
/// automatically dispatch to the thread local (`Hub::current`) Hub. In some
/// situations this might not be possible, in which case it might become
/// necessary to manually work with the Hub. This is for instance the case when
/// working with async code.
///
/// Hubs that are wrapped in `Arc`s can be bound to the current thread with
/// the `run` static method.
#[derive(Clone)]
pub struct Hub {}

impl Hub {
    /// Creates a new hub from the given client and scope.
    pub fn new(client: Option<Client>, scope: Scope) -> Hub {
        let _ = (client, scope);
        todo!()
    }

    /// Creates a new hub based on the top scope of the given hub.
    pub fn new_from_top<H: AsRef<Hub>>(other: H) -> Hub {
        let _ = other;
        todo!()
    }

    /// Returns the current thread local hub.
    pub fn current() -> Hub {
        Hub::with(Clone::clone)
    }

    /// Returns the main thread's hub.
    ///
    /// This is similar to `current` but instead of picking the current
    /// thread's hub it returns the main thread's hub instead.
    pub fn main() -> Hub {
        todo!()
    }

    /// Invokes the callback with the current hub.
    ///
    /// This is a slightly more efficient version than `Hub::current()`, as it
    /// avoids a `clone`.
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Hub) -> R,
    {
        let _ = f;
        todo!()
    }

    /// Like `Hub::with` but only calls the function if a client is bound.
    ///
    /// This is useful for integrations that want to do efficiently nothing if
    /// there is no client bound. Additionally this internally ensures that the
    /// client can be safely synchronized.
    /// This prevents accidental recursive calls into the client.
    pub fn with_active<F, R>(f: F) -> R
    where
        F: FnOnce(&Hub) -> R,
        R: Default,
    {
        let _ = f;
        todo!()
    }

    /// Binds the hub to the current thread for the duration of the callback.
    pub fn run<F: FnOnce() -> R, R>(&self, f: F) -> R {
        let _ = f;
        todo!()
    }

    /// Sends the event to the current client with the current scope.
    ///
    /// See the global [`capture_event`](fn.capture_event.html)
    /// for more documentation.
    pub fn capture_event(&self, event: Event<'static>) -> Option<Uuid> {
        let _ = event;
        todo!()
    }

    /// Captures an arbitrary message.
    ///
    /// See the global [`capture_message`](fn.capture_message.html)
    /// for more documentation.
    pub fn capture_message(&self, msg: &str, level: Level) -> Option<Uuid> {
        let event = Event {
            message: Some(msg.to_string()),
            level,
            ..Default::default()
        };
        self.capture_event(event)
    }

    /// Invokes a function that can modify the current scope.
    ///
    /// See the global [`configure_scope`](fn.configure_scope.html)
    /// for more documentation.
    pub fn configure_scope<F, R>(&self, f: F) -> R
    where
        R: Default,
        F: FnOnce(&mut Scope) -> R,
    {
        let _ = f;
        todo!()
    }

    /// Pushes a new scope.
    ///
    /// This returns a guard that when dropped will pop the scope again.
    pub fn push_scope(&self) -> ScopeGuard {
        todo!()
    }

    /// Temporarily pushes a scope for a single call optionally reconfiguring it.
    ///
    /// In case no client is bound, the given `callback` is still invoked, but
    /// `scope_config` is not.
    pub fn with_scope<C, F, R>(&self, scope_config: C, callback: F) -> R
    where
        C: FnOnce(&mut Scope),
        F: FnOnce() -> R,
    {
        let _ = (scope_config, callback);
        todo!()
    }

    /// Adds a new breadcrumb to the current scope.
    ///
    /// See the global [`add_breadcrumb`](fn.add_breadcrumb.html)
    /// for more documentation.
    pub fn add_breadcrumb<B: IntoBreadcrumbs>(&self, breadcrumbs: B) {
        let _ = breadcrumbs;
        todo!()
    }

    /// Returns the currently bound client.
    pub fn client(&self) -> Option<Client> {
        todo!()
    }

    /// Binds a new client to the hub.
    pub fn bind_client(&self, client: Option<Client>) {
        let _ = client;
        todo!()
    }

    /// Returns the last event id.
    pub fn last_event_id(&self) -> Option<Uuid> {
        todo!()
    }
}
