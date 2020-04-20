use std::cell::UnsafeCell;
use std::fmt;
use std::sync::{Arc, PoisonError, RwLock, TryLockError};
use std::thread;

use crate::{stack::Stack, Client, Event, IntoBreadcrumbs, Level, Scope, Uuid};

lazy_static::lazy_static! {
    static ref PROCESS_HUB: (Hub, thread::ThreadId) = (
        Hub::new(None, Default::default()),
        thread::current().id()
    );
}

//static USE_PROCESS_HUB: Cell<bool> = Cell::new(PROCESS_HUB.1 == thread::current().id());
thread_local! {
    static THREAD_HUB: UnsafeCell<Hub> = UnsafeCell::new(
        if PROCESS_HUB.1 == thread::current().id() {
            PROCESS_HUB.0.clone()
        } else {
            Hub::new_from_top(&PROCESS_HUB.0)
        }
    );
}

/// A scope guard which is returned from [`Hub::push_scope`](struct.Hub.html#method.push_scope)
pub struct ScopeGuard(pub(crate) Option<(Hub, usize)>);

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        if let Some((hub, depth)) = self.0.take() {
            hub.with_stack_mut(|stack| {
                if stack.depth() != depth {
                    panic!("Tried to pop guards out of order");
                }
                stack.pop();
            })
        }
    }
}

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
/// Toplevel convenience functions are exposed that will
/// automatically dispatch to the thread local Hub
/// ([`Hub::current`](struct.Hub.html#method.current)).
/// The thread local hub can be temporarily changed using
/// [`run`](struct.Hub.html#method.run).
#[derive(Clone)]
pub struct Hub(Arc<HubInner>);

struct HubInner {
    stack: RwLock<Stack>,
    last_event_id: RwLock<Option<Uuid>>,
}

impl Hub {
    fn with_stack<F: FnOnce(&Stack) -> R, R>(&self, f: F) -> R {
        let guard = self.0.stack.read().unwrap_or_else(PoisonError::into_inner);
        f(&*guard)
    }

    fn with_stack_mut<F: FnOnce(&mut Stack) -> R, R>(&self, f: F) -> R {
        let mut guard = self.0.stack.write().unwrap_or_else(PoisonError::into_inner);
        f(&mut *guard)
    }

    fn is_active_and_usage_safe(&self) -> bool {
        let guard = match self.0.stack.try_read() {
            Err(TryLockError::Poisoned(err)) => err.into_inner(),
            Err(TryLockError::WouldBlock) => return false,
            Ok(guard) => guard,
        };
        guard.top().client.is_some()
    }

    /// Creates a new hub from the given client and scope.
    pub fn new(client: Option<Client>, scope: Scope) -> Hub {
        Hub(Arc::new(HubInner {
            stack: RwLock::new(Stack::from_client_and_scope(client, scope)),
            last_event_id: RwLock::new(None),
        }))
    }

    /// Creates a new hub based on the top scope of the given hub.
    pub fn new_from_top(other: &Hub) -> Hub {
        other.with_stack(|stack| {
            let top = stack.top();
            Hub::new(top.client.clone(), top.scope.clone())
        })
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
        PROCESS_HUB.0.clone()
    }

    /// Invokes the callback with the current hub.
    ///
    /// This is a slightly more efficient version than `Hub::current()`, as it
    /// avoids a `clone`.
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Hub) -> R,
    {
        // not on safety: this is safe because even though we change the Arc
        // by temorary binding we guarantee that the original Arc stays alive.
        // For more information see: run
        THREAD_HUB.with(|uc| {
            let hub = unsafe { &*uc.get() };
            f(hub)
        })
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
        Hub::with(|hub| {
            if hub.is_active_and_usage_safe() {
                f(hub)
            } else {
                Default::default()
            }
        })
    }

    /// Binds the hub to the current thread for the duration of the callback.
    pub fn run<F: FnOnce() -> R, R>(&self, f: F) -> R {
        THREAD_HUB.with(|uc| {
            let ptr = uc.get();
            let other = unsafe { &*ptr };
            if Arc::ptr_eq(&self.0, &other.0) {
                // `self` is already the thread local hub, so call the function
                // directly
                return f();
            }
            unsafe {
                use std::panic;

                let old = (*ptr).clone();
                *ptr = self.clone();

                let rv = panic::catch_unwind(panic::AssertUnwindSafe(f));
                *ptr = old;
                match rv {
                    Err(err) => panic::resume_unwind(err),
                    Ok(rv) => rv,
                }
            }
        })
    }

    /// Sends the event to the current client with the current scope.
    ///
    /// See the global [`capture_event`](fn.capture_event.html)
    /// for more documentation.
    pub fn capture_event(&self, event: Event<'static>) -> Option<Uuid> {
        self.with_stack(|stack| {
            let top = stack.top();
            let client = top.client.as_ref()?;

            let event_id = client.capture_event(event, Some(&top.scope));
            if let Some(event_id) = event_id {
                *self.0.last_event_id.write().unwrap() = Some(event_id);
            }
            event_id
        })
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
        self.with_stack_mut(|stack| {
            let top = stack.top_mut();
            if top.client.is_none() {
                return Default::default();
            }
            f(&mut top.scope)
        })
    }

    /// Pushes a new scope.
    ///
    /// This returns a guard that when dropped will pop the scope again.
    pub fn push_scope(&self) -> ScopeGuard {
        self.with_stack_mut(|stack| {
            stack.push();
            ScopeGuard(Some((self.clone(), stack.depth())))
        })
    }

    /// Temporarily pushes a scope for a single call optionally reconfiguring it.
    ///
    /// See the global [`with_scope`](fn.with_scope.html)
    /// for more documentation.
    pub fn with_scope<C, F, R>(&self, scope_config: C, callback: F) -> R
    where
        C: FnOnce(&mut Scope),
        F: FnOnce() -> R,
    {
        let _guard = self.push_scope();
        self.configure_scope(scope_config);
        callback()
    }

    /// Adds a new breadcrumb to the current scope.
    ///
    /// See the global [`add_breadcrumb`](fn.add_breadcrumb.html)
    /// for more documentation.
    pub fn add_breadcrumb<B: IntoBreadcrumbs>(&self, breadcrumbs: B) {
        self.with_stack_mut(|stack| {
            let top = stack.top_mut();
            let scope = &mut top.scope;
            if let Some(client) = &top.client {
                let options = &client.options;
                for breadcrumb in breadcrumbs.into_breadcrumbs() {
                    let breadcrumb_opt = match &options.before_breadcrumb {
                        Some(callback) => callback(breadcrumb),
                        None => Some(breadcrumb),
                    };
                    if let Some(breadcrumb) = breadcrumb_opt {
                        scope.breadcrumbs.push_back(breadcrumb);
                    }
                    while scope.breadcrumbs.len() > options.max_breadcrumbs {
                        scope.breadcrumbs.pop_front();
                    }
                }
            }
        })
    }

    /// Returns the currently bound client.
    pub fn client(&self) -> Option<Client> {
        self.with_stack(|stack| stack.top().client.clone())
    }

    /// Binds a new client to the hub.
    pub fn bind_client(&self, client: Option<Client>) {
        self.with_stack_mut(|stack| {
            stack.top_mut().client = client;
        })
    }

    /// Returns the last event id.
    pub fn last_event_id(&self) -> Option<Uuid> {
        *self.0.last_event_id.read().unwrap()
    }
}
