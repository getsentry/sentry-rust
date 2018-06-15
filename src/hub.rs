use std::iter;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
#[allow(unused)]
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
#[allow(unused)]
use std::thread;
use std::time::Duration;

use client::Client;
use protocol::{Breadcrumb, Event, Level};
use scope::{Scope, ScopeGuard, ScopeHandle};

#[cfg(feature = "with_client_implementation")]
use scope::{Stack, StackLayerToken};

use uuid::Uuid;

#[cfg(feature = "with_client_implementation")]
lazy_static! {
    static ref PROCESS_HUB: (Arc<Hub>, thread::ThreadId) = (
        Arc::new(Hub::new(None, Arc::new(Default::default()))),
        thread::current().id()
    );
}
#[cfg(feature = "with_client_implementation")]
thread_local! {
    static THREAD_HUB: Arc<Hub> = Arc::new(Hub::new_from_top(&PROCESS_HUB.0));
}

/// A helper trait that converts an object into a breadcrumb.
pub trait IntoBreadcrumbs {
    /// The iterator type for the breadcrumbs.
    type Output: Iterator<Item = Breadcrumb>;

    /// This converts the object into an optional breadcrumb.
    fn into_breadcrumbs(self) -> Self::Output;
}

impl IntoBreadcrumbs for Breadcrumb {
    type Output = iter::Once<Breadcrumb>;

    fn into_breadcrumbs(self) -> Self::Output {
        return iter::once(self);
    }
}

impl IntoBreadcrumbs for Vec<Breadcrumb> {
    type Output = ::std::vec::IntoIter<Breadcrumb>;

    fn into_breadcrumbs(self) -> Self::Output {
        self.into_iter()
    }
}

impl IntoBreadcrumbs for Option<Breadcrumb> {
    type Output = ::std::option::IntoIter<Breadcrumb>;

    fn into_breadcrumbs(self) -> Self::Output {
        self.into_iter()
    }
}

impl<F: FnOnce() -> I, I: IntoBreadcrumbs> IntoBreadcrumbs for F {
    type Output = I::Output;

    fn into_breadcrumbs(self) -> Self::Output {
        self().into_breadcrumbs()
    }
}

#[doc(hidden)]
pub struct PendingProcessors(Vec<Box<FnOnce() -> Box<Fn(&mut Event) + Send + Sync>>>);

// this is not actually sync but our uses of it are
unsafe impl Sync for PendingProcessors {}
unsafe impl Send for PendingProcessors {}

struct HubImpl {
    stack: RwLock<Stack>,
    pending_processors: RwLock<PendingProcessors>,
    has_pending_processors: AtomicBool,
}

/// Different scope stack implementations for the hub.
pub trait ScopeStack {
    #[doc(hidden)]
    fn with<F: FnOnce(&Stack) -> R, R>(&self, f: F) -> R;
    #[doc(hidden)]
    fn with_mut<F: FnOnce(&mut Stack) -> R, R>(&self, f: F) -> R;
    #[doc(hidden)]
    fn with_processors_mut<F: FnOnce(&mut PendingProcessors) -> R, R>(&self, f: F) -> R;
    #[doc(hidden)]
    fn is_active_and_usage_safe(&self) -> bool;
}

impl HubImpl {
    fn with<F: FnOnce(&Stack) -> R, R>(&self, f: F) -> R {
        let guard = self.stack.read().unwrap_or_else(|x| x.into_inner());
        f(&*guard)
    }

    fn with_mut<F: FnOnce(&mut Stack) -> R, R>(&self, f: F) -> R {
        let mut guard = self.stack.write().unwrap_or_else(|x| x.into_inner());
        f(&mut *guard)
    }

    fn with_processors_mut<F: FnOnce(&mut PendingProcessors) -> R, R>(&self, f: F) -> R {
        f(&mut *self.pending_processors
            .write()
            .unwrap_or_else(|x| x.into_inner()))
    }

    fn is_active_and_usage_safe(&self) -> bool {
        let guard = match self.stack.try_read() {
            Err(TryLockError::Poisoned(err)) => err.into_inner(),
            Err(TryLockError::WouldBlock) => return false,
            Ok(guard) => guard,
        };
        guard.top().client.is_some()
    }
}

/// The central object that can manages scopes and clients.
///
/// This can be used to capture events and manage the scope.  This object is
/// internally synchronized so it can be used from multiple threads if needed.
/// The default hub that is available automatically is thread local.
///
/// In most situations developers do not need to interface the hub.  Instead
/// toplevel convenience functions are expose tht will automatically dispatch
/// to global (`Hub::current`) hub.  In some situations this might not be
/// possible in which case it might become necessary to manually work with the
/// hub.  This is for instance the case when working with async code.
///
/// Most common operations:
///
/// * `Hub::new`: creates a brand new hub
/// * `Hub::current`: returns the default hub
/// * `Hub::with`: invoke a callback with the default hub
/// * `Hub::with_active`: like `Hub::with` but does not invoke the callback if
///   the client is not in a supported state or not bound
/// * `Hub::derive`: creates a new hub with just the top scope
pub struct Hub {
    #[cfg(feature = "with_client_implementation")]
    inner: HubImpl,
}

impl Hub {
    /// Creates a new hub from the given client and scope.
    #[cfg(feature = "with_client_implementation")]
    pub fn new(client: Option<Arc<Client>>, scope: Arc<Scope>) -> Hub {
        Hub {
            inner: HubImpl {
                stack: RwLock::new(Stack::from_client_and_scope(client, scope)),
                pending_processors: RwLock::new(PendingProcessors(vec![])),
                has_pending_processors: AtomicBool::new(false),
            },
        }
    }

    /// Creates a new hub based on the top scope of the given hub.
    #[cfg(feature = "with_client_implementation")]
    pub fn new_from_top<H: AsRef<Hub>>(other: H) -> Hub {
        let hub = other.as_ref();
        hub.flush_pending_processors();
        hub.inner.with(|stack| {
            let top = stack.top();
            Hub::new(top.client.clone(), top.scope.clone())
        })
    }

    /// Returns the default hub.
    ///
    /// This method is unavailable if the client implementation is disabled.
    /// For shim-only usage use `Hub::with_active`.
    #[cfg(feature = "with_client_implementation")]
    pub fn current() -> Arc<Hub> {
        Hub::with(|hub| hub.clone())
    }

    /// Returns the main thread's hub.
    ///
    /// This is similar to `current` but instead of picking the current
    /// thread's hub it returns the main thread's hub instead.
    #[cfg(feature = "with_client_implementation")]
    pub fn main() -> Arc<Hub> {
        PROCESS_HUB.0.clone()
    }

    /// Invokes the callback with the global hub.
    ///
    /// This is a slightly more efficient version than `Hub::current()` and
    /// also unavailable in shim-only mode.
    #[cfg(feature = "with_client_implementation")]
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Arc<Hub>) -> R,
    {
        if thread::current().id() == PROCESS_HUB.1 {
            f(&PROCESS_HUB.0)
        } else {
            THREAD_HUB.with(|stack| f(&*stack))
        }
    }

    /// Like `Hub::with` but only calls the function if a client is bound.
    ///
    /// This is useful for integrations that want to do efficiently nothing if there is no
    /// client bound.  Additionally this internally ensures that the client can be safely
    /// synchronized.  This prevents accidental recursive calls into the client.
    #[allow(unused_variables)]
    pub fn with_active<F, R>(f: F) -> R
    where
        F: FnOnce(&Arc<Hub>) -> R,
        R: Default,
    {
        with_client_impl! {{
            Hub::with(|hub| {
                if hub.is_active_and_usage_safe() {
                    f(hub)
                } else {
                    Default::default()
                }
            })
        }}
    }
}

impl Hub {
    /// Sends the event to the current client with the current scope.
    ///
    /// In case no client is bound this does nothing instead.
    #[allow(unused_variables)]
    pub fn capture_event(&self, event: Event<'static>) -> Uuid {
        self.flush_pending_processors();
        with_client_impl! {{
            self.inner.with(|stack| {
                let top = stack.top();
                if let Some(ref client) = top.client {
                    client.capture_event(event, Some(&top.scope))
                } else {
                    Default::default()
                }
            })
        }}
    }

    /// Captures an arbitrary message.
    pub fn capture_message(&self, msg: &str, level: Level) -> Uuid {
        self.capture_event(Event {
            message: Some(msg.to_string()),
            level,
            ..Default::default()
        })
    }

    /// Drains the currently pending events.
    #[allow(unused_variables)]
    pub fn drain_events(&self, timeout: Option<Duration>) {
        with_client_impl! {{
            if let Some(ref client) = self.client() {
                client.drain_events(timeout);
            }
        }}
    }

    /// Returns the currently bound client.
    pub fn client(&self) -> Option<Arc<Client>> {
        with_client_impl! {{
            self.inner.with(|stack| {
                stack.top().client.clone()
            })
        }}
    }

    /// Binds a new client to the hub.
    #[cfg(feature = "with_client_implementation")]
    pub fn bind_client(&self, client: Option<Arc<Client>>) {
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                stack.top_mut().client = client;
            })
        }}
    }

    /// Pushes a new scope.
    ///
    /// This returns a guard that when dropped will pop the scope again.
    pub fn push_scope(&self) -> ScopeGuard {
        self.flush_pending_processors();
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                stack.push();
                ScopeGuard(Some(stack.layer_token()))
            })
        }}
    }

    /// Invokes a function that can modify the current scope.
    #[allow(unused_variables)]
    pub fn configure_scope<F, R>(&self, f: F) -> R
    where
        R: Default,
        F: FnOnce(&mut Scope) -> R,
    {
        with_client_impl! {{
            let (new_scope, rv) = self.with_scope(|scope| {
                let mut new_scope = (**scope).clone();
                let rv = f(&mut new_scope);
                (new_scope, rv)
            });
            self.with_scope_mut(|ptr| *ptr = new_scope);
            rv
        }}
    }

    /// Adds a new breadcrumb to the current scope.
    ///
    /// This is equivalent to the global [`sentry::add_breadcrumb`](fn.add_breadcrumb.html) but
    /// sends the breadcrumb into the hub instead.
    #[allow(unused_variables)]
    pub fn add_breadcrumb<B: IntoBreadcrumbs>(&self, breadcrumb: B) {
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                let top = stack.top_mut();
                if let Some(ref client) = top.client {
                    let scope = Arc::make_mut(&mut top.scope);
                    let limit = client.options().max_breadcrumbs;
                    for breadcrumb in breadcrumb.into_breadcrumbs() {
                    scope.breadcrumbs = scope.breadcrumbs.push_back(breadcrumb);
                        while scope.breadcrumbs.len() > limit {
                            if let Some((_, new)) = scope.breadcrumbs.pop_front() {
                                scope.breadcrumbs = new;
                            }
                        }
                    }
                }
            })
        }}
    }

    /// Returns the handle to the current scope.
    ///
    /// This can be used to propagate a scope to another thread easily.  The
    /// parent thread retrieves a handle and the child thread binds it. A handle
    /// can be cloned so that it can be used in multiple threads.
    ///
    /// ## Example
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
    pub fn scope_handle(&self) -> ScopeHandle {
        with_client_impl! {{
            self.with_scope(|scope| ScopeHandle(Some(scope.clone())))
        }}
    }

    /// Registers an event processor with the topmost scope.
    ///
    /// An event processor here is returned by an `FnOnce()` function that returns a
    /// function taking an Event that needs to be `Send + Sync`.  By having the
    /// function be a factory for the actual event processor the outer function does
    /// not have to be `Sync` as sentry will execute it before a hub crosses a thread
    /// boundary.
    pub fn add_event_processor(&self, f: Box<FnOnce() -> Box<Fn(&mut Event) + Send + Sync>>) {
        with_client_impl! {{
            self.inner.with_processors_mut(|pending| {
                pending.0.push(f);
            });
            self.inner.has_pending_processors.store(true, Ordering::AcqRel);
        }}
    }

    fn flush_pending_processors(&self) {
        with_client_impl! {{
            if !self.inner.has_pending_processors.load(Ordering::AcqRel) {
                return;
            }
            let mut new_processors = vec![];
            self.inner.with_processors_mut(|pending| {
                for processor in pending.0.iter_mut() {
                    let processor: &mut Box<FnMut() -> Box<Fn(&mut Event) + Send + Sync>> = unsafe {
                        mem::transmute(processor)
                    };
                    new_processors.push(processor());
                }
                pending.0.clear();
            });
            self.inner.has_pending_processors.store(false, Ordering::AcqRel);
            if !new_processors.is_empty() {
                self.configure_scope(|scope| {
                    for func in new_processors.into_iter() {
                        scope.event_processors = scope.event_processors.push_back(func);
                    }
                });
            }
        }}
    }

    #[cfg(feature = "with_client_implementation")]
    pub(crate) fn is_active_and_usage_safe(&self) -> bool {
        self.inner.is_active_and_usage_safe()
    }

    #[cfg(feature = "with_client_implementation")]
    pub(crate) fn with_scope<F: FnOnce(&Arc<Scope>) -> R, R>(&self, f: F) -> R {
        self.inner.with(|stack| f(&stack.top().scope))
    }

    #[cfg(feature = "with_client_implementation")]
    pub(crate) fn with_scope_mut<F: FnOnce(&mut Scope) -> R, R>(&self, f: F) -> R {
        self.inner
            .with_mut(|stack| f(Arc::make_mut(&mut stack.top_mut().scope)))
    }

    #[cfg(feature = "with_client_implementation")]
    pub(crate) fn pop_scope(&self, token: StackLayerToken) {
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                if stack.layer_token() != token {
                    panic!("Current active stack does not match scope guard");
                }
                stack.pop();
            })
        }}
    }
}
