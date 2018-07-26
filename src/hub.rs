#[allow(unused)]
use std::cell::{Cell, UnsafeCell};
use std::iter;
use std::mem;
#[allow(unused)]
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
#[allow(unused)]
use std::thread;
use std::time::Duration;

#[cfg(feature = "with_client_implementation")]
use fragile::SemiSticky;

#[allow(unused)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "with_client_implementation")]
use client::Client;
use protocol::{Breadcrumb, Event, Level};
use scope::{Scope, ScopeGuard};

#[cfg(feature = "with_client_implementation")]
use scope::{Stack, StackLayerToken};

#[cfg(feature = "with_backtrace")]
use backtrace_support::current_stacktrace;

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
    static THREAD_HUB: UnsafeCell<Arc<Hub>> = UnsafeCell::new(
        Arc::new(Hub::new_from_top(&PROCESS_HUB.0)));
    static USE_PROCESS_HUB: Cell<bool> = Cell::new(PROCESS_HUB.1 == thread::current().id());
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

#[cfg(feature = "with_client_implementation")]
pub(crate) trait EventProcessorFactoryFn {
    fn call(self: Box<Self>) -> Box<Fn(&mut Event) + Send + Sync>;
}

#[cfg(feature = "with_client_implementation")]
pub(crate) enum PendingProcessor {
    Send(Box<EventProcessorFactoryFn + Send + Sync>),
    NonSend(SemiSticky<Box<EventProcessorFactoryFn>>),
}

#[cfg(feature = "with_client_implementation")]
impl<F: 'static + FnOnce() -> Box<Fn(&mut Event) + Send + Sync>> EventProcessorFactoryFn for F {
    fn call(self: Box<Self>) -> Box<Fn(&mut Event) + Send + Sync> {
        let this: Self = *self;
        this()
    }
}

#[cfg(feature = "with_client_implementation")]
impl PendingProcessor {
    fn is_safe_call(&self) -> bool {
        match *self {
            PendingProcessor::Send(..) => true,
            PendingProcessor::NonSend(ref f) => f.is_valid(),
        }
    }

    fn call(self) -> Box<Fn(&mut Event) + Send + Sync> {
        match self {
            PendingProcessor::Send(f) => f.call(),
            PendingProcessor::NonSend(f) => f.into_inner().call(),
        }
    }
}

#[cfg(feature = "with_client_implementation")]
struct HubImpl {
    stack: RwLock<Stack>,
    pending_processors: Mutex<Vec<PendingProcessor>>,
    has_pending_processors: AtomicBool,
}

#[cfg(feature = "with_client_implementation")]
impl HubImpl {
    fn with<F: FnOnce(&Stack) -> R, R>(&self, f: F) -> R {
        let guard = self.stack.read().unwrap_or_else(|x| x.into_inner());
        f(&*guard)
    }

    fn with_mut<F: FnOnce(&mut Stack) -> R, R>(&self, f: F) -> R {
        let mut guard = self.stack.write().unwrap_or_else(|x| x.into_inner());
        f(&mut *guard)
    }

    fn with_processors_mut<F: FnOnce(&mut Vec<PendingProcessor>) -> R, R>(&self, f: F) -> R {
        f(&mut *self.pending_processors
            .lock()
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
/// Hubs that are wrapped in `Arc`s can be bound to the current thread with
/// the `run` static method.
///
/// Most common operations:
///
/// * `Hub::new`: creates a brand new hub
/// * `Hub::current`: returns the thread local hub
/// * `Hub::with`: invoke a callback with the thread local hub
/// * `Hub::with_active`: like `Hub::with` but does not invoke the callback if
///   the client is not in a supported state or not bound
/// * `Hub::new_from_top`: creates a new hub with just the top scope of another hub.
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
                pending_processors: Mutex::new(vec![]),
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

    /// Returns the current hub.
    ///
    /// By default each thread gets a different thread local hub.  If an
    /// atomically reference counted hub is available it can override this
    /// one here by calling `Hub::run` with a closure.
    ///
    /// This method is unavailable if the client implementation is disabled.
    /// When using the minimal API set use `Hub::with_active` instead.
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

    /// Invokes the callback with the default hub.
    ///
    /// This is a slightly more efficient version than `Hub::current()` and
    /// also unavailable in minimal mode.
    #[cfg(feature = "with_client_implementation")]
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Arc<Hub>) -> R,
    {
        if USE_PROCESS_HUB.with(|x| x.get()) {
            f(&PROCESS_HUB.0)
        } else {
            // not on safety: this is safe because even though we change the Arc
            // by temorary binding we guarantee that the original Arc stays alive.
            // For more information see: run
            THREAD_HUB.with(|stack| unsafe {
                let ptr = stack.get();
                f(&*ptr)
            })
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

    /// Binds a hub to the current thread for the duration of the call.
    #[cfg(feature = "with_client_implementation")]
    pub fn run<F: FnOnce() -> R, R>(hub: Arc<Hub>, f: F) -> R {
        hub.flush_pending_processors();
        let mut restore_process_hub = false;
        let did_switch = THREAD_HUB.with(|ctx| unsafe {
            let ptr = ctx.get();
            if &**ptr as *const _ == &*hub as *const _ {
                None
            } else {
                USE_PROCESS_HUB.with(|x| {
                    if x.get() {
                        restore_process_hub = true;
                        x.set(false);
                    }
                });
                let old = (*ptr).clone();
                *ptr = hub.clone();
                Some(old)
            }
        });

        match did_switch {
            None => {
                // None means no switch happened.  We can invoke the function
                // just like that, no changes necessary.
                f()
            }
            Some(old_hub) => {
                use std::panic;

                // this is for the case where we just switched the hub.  This
                // means we need to catch the panic, restore the
                // old context and resume the panic if needed.
                let rv = panic::catch_unwind(panic::AssertUnwindSafe(|| f()));
                THREAD_HUB.with(|ctx| unsafe { *ctx.get() = old_hub });
                if restore_process_hub {
                    USE_PROCESS_HUB.with(|x| x.set(true));
                }
                match rv {
                    Err(err) => panic::resume_unwind(err),
                    Ok(rv) => rv,
                }
            }
        }
    }

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
        with_client_impl! {{
            self.inner.with(|stack| {
                let top = stack.top();
                if let Some(ref client) = top.client {
                    let mut event = Event {
                        message: Some(msg.to_string()),
                        level,
                        ..Default::default()
                    };
                    #[cfg(feature = "with_backtrace")] {
                        use protocol::Thread;
                        if client.options().attach_stacktrace {
                            let thread_id: u64 = unsafe {
                                mem::transmute(thread::current().id())
                            };
                            event.threads.push(Thread {
                                id: Some(thread_id.to_string().into()),
                                name: thread::current().name().map(|x| x.to_string()),
                                current: true,
                                stacktrace: current_stacktrace(),
                                ..Default::default()
                            })
                        }
                    }
                    self.capture_event(event)
                } else {
                    Uuid::nil()
                }
            })
        }}
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
    #[cfg(feature = "with_client_implementation")]
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

    /// Registers an event processor with the topmost scope.
    ///
    /// An event processor here is returned by an `FnOnce()` function that returns a
    /// function taking an Event that needs to be `Send + Sync`.  By having the
    /// function be a factory for the actual event processor the outer function does
    /// not have to be `Sync` as sentry will execute it before a hub crosses a thread
    /// boundary.
    ///
    /// # Threading
    ///
    /// If a hub is used from multiple threads event processors might not be executed
    /// if another thread triggers an event processor first.  For such cases
    /// `add_send_event_processor` must be used instead.  This means that if thread 1
    /// will add the processor but thread 2 will send the next event, then processors
    /// are not run until thread 1 sends an event.  Processors are also triggered
    /// if a scope is pushed or a hub is created from this hub via `Hub::new_from_top`.
    #[allow(unused)]
    pub fn add_event_processor<F: FnOnce() -> Box<Fn(&mut Event) + Send + Sync> + 'static>(
        &self,
        f: F,
    ) {
        with_client_impl! {{
            self.inner.with_processors_mut(|pending| {
                pending.push(PendingProcessor::NonSend(SemiSticky::new(
                    Box::new(f) as Box<EventProcessorFactoryFn>)));
            });
            self.inner.has_pending_processors.store(true, Ordering::Release);
        }}
    }

    /// Registers a sendable event processor with the topmost scope.
    ///
    /// This works like `add_event_processor` but registers functions that are `Send`
    /// which permits them to be used from multiple threads.  If a hub is used from
    /// multiple threads at once then only sendable event processors will be guaranteed
    /// to run.
    #[allow(unused)]
    pub fn add_send_event_processor<
        F: FnOnce() -> Box<Fn(&mut Event) + Send + Sync> + Send + Sync + 'static,
    >(
        &self,
        f: F,
    ) {
        with_client_impl! {{
            use std::mem;
            self.inner.with_processors_mut(|pending| {
                pending.push(PendingProcessor::Send(
                    Box::new(f) as Box<EventProcessorFactoryFn + Send + Sync>));
            });
            self.inner.has_pending_processors.store(true, Ordering::Release);
        }}
    }

    fn flush_pending_processors(&self) {
        with_client_impl! {{
            if !self.inner.has_pending_processors.load(Ordering::Acquire) {
                return;
            }
            let mut new_processors = vec![];
            let any_left = self.inner.with_processors_mut(|vec| {
                let mut i = 0;
                while i < vec.len() {
                    if !vec[i].is_safe_call() {
                        i += 1;
                    } else {
                        new_processors.push(vec.remove(i).call());
                    }
                }
                !vec.is_empty()
            });
            self.inner.has_pending_processors.store(any_left, Ordering::Release);
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
