use std::cell::{Cell, UnsafeCell};
use std::sync::{Arc, PoisonError, RwLock};
use std::thread;

use crate::Scope;
use crate::{scope::Stack, Client, Hub};

use once_cell::sync::Lazy;

static PROCESS_HUB: Lazy<(Arc<Hub>, thread::ThreadId)> = Lazy::new(|| {
    (
        Arc::new(Hub::new(None, Arc::new(Default::default()))),
        thread::current().id(),
    )
});

thread_local! {
    static THREAD_HUB: UnsafeCell<Arc<Hub>> = UnsafeCell::new(
        Arc::new(Hub::new_from_top(&PROCESS_HUB.0)));
    static USE_PROCESS_HUB: Cell<bool> = Cell::new(PROCESS_HUB.1 == thread::current().id());
}

#[derive(Debug)]
pub(crate) struct HubImpl {
    pub(crate) stack: Arc<RwLock<Stack>>,
}

impl HubImpl {
    pub(crate) fn with<F: FnOnce(&Stack) -> R, R>(&self, f: F) -> R {
        let guard = self.stack.read().unwrap_or_else(PoisonError::into_inner);
        f(&guard)
    }

    pub(crate) fn with_mut<F: FnOnce(&mut Stack) -> R, R>(&self, f: F) -> R {
        let mut guard = self.stack.write().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard)
    }

    pub(crate) fn is_active_and_usage_safe(&self) -> bool {
        let guard = match self.stack.read() {
            Err(err) => err.into_inner(),
            Ok(guard) => guard,
        };

        guard
            .top()
            .client
            .as_ref()
            .map_or(false, |c| c.is_enabled())
    }
}

impl Hub {
    /// Creates a new hub from the given client and scope.
    pub fn new(client: Option<Arc<Client>>, scope: Arc<Scope>) -> Hub {
        Hub {
            inner: HubImpl {
                stack: Arc::new(RwLock::new(Stack::from_client_and_scope(client, scope))),
            },
            last_event_id: RwLock::new(None),
        }
    }

    /// Creates a new hub based on the top scope of the given hub.
    pub fn new_from_top<H: AsRef<Hub>>(other: H) -> Hub {
        let hub = other.as_ref();
        hub.inner.with(|stack| {
            let top = stack.top();
            Hub::new(top.client.clone(), top.scope.clone())
        })
    }

    /// Returns the current, thread-local hub.
    ///
    /// Invoking this will return the current thread-local hub.  The first
    /// time it is called on a thread, a new thread-local hub will be
    /// created based on the topmost scope of the hub on the main thread as
    /// returned by [`Hub::main`].  If the main thread did not yet have a
    /// hub it will be created when invoking this function.
    ///
    /// To have control over which hub is installed as the current
    /// thread-local hub, use [`Hub::run`].
    ///
    /// This method is unavailable if the client implementation is disabled.
    /// When using the minimal API set use [`Hub::with_active`] instead.
    pub fn current() -> Arc<Hub> {
        Hub::with(Arc::clone)
    }

    /// Returns the main thread's hub.
    ///
    /// This is similar to [`Hub::current`] but instead of picking the
    /// current thread's hub it returns the main thread's hub instead.
    pub fn main() -> Arc<Hub> {
        PROCESS_HUB.0.clone()
    }

    /// Invokes the callback with the default hub.
    ///
    /// This is a slightly more efficient version than [`Hub::current`] and
    /// also unavailable in minimal mode.
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Arc<Hub>) -> R,
    {
        if USE_PROCESS_HUB.with(Cell::get) {
            f(&PROCESS_HUB.0)
        } else {
            // note on safety: this is safe because even though we change the Arc
            // by temporary binding we guarantee that the original Arc stays alive.
            // For more information see: run
            THREAD_HUB.with(|stack| unsafe {
                let ptr = stack.get();
                f(&*ptr)
            })
        }
    }

    /// Binds a hub to the current thread for the duration of the call.
    ///
    /// During the execution of `f` the given hub will be installed as the
    /// thread-local hub.  So any call to [`Hub::current`] during this time
    /// will return the provided hub.
    ///
    /// Once the function is finished executing, including after it
    /// paniced, the original hub is re-installed if one was present.
    pub fn run<F: FnOnce() -> R, R>(hub: Arc<Hub>, f: F) -> R {
        let mut restore_process_hub = false;
        let did_switch = THREAD_HUB.with(|ctx| unsafe {
            let ptr = ctx.get();
            if std::ptr::eq(&**ptr, &*hub) {
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
                let rv = panic::catch_unwind(panic::AssertUnwindSafe(f));
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

    /// Returns the currently bound client.
    pub fn client(&self) -> Option<Arc<Client>> {
        self.inner.with(|stack| stack.top().client.clone())
    }

    /// Binds a new client to the hub.
    pub fn bind_client(&self, client: Option<Arc<Client>>) {
        self.inner.with_mut(|stack| {
            stack.top_mut().client = client;
        })
    }

    pub(crate) fn is_active_and_usage_safe(&self) -> bool {
        self.inner.is_active_and_usage_safe()
    }

    pub(crate) fn with_current_scope<F: FnOnce(&Scope) -> R, R>(&self, f: F) -> R {
        self.inner.with(|stack| f(&stack.top().scope))
    }

    pub(crate) fn with_current_scope_mut<F: FnOnce(&mut Scope) -> R, R>(&self, f: F) -> R {
        self.inner
            .with_mut(|stack| f(Arc::make_mut(&mut stack.top_mut().scope)))
    }
}
