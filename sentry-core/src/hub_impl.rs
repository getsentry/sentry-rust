use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::{Arc, LazyLock, MutexGuard, PoisonError, RwLock};
use std::thread;
use std::thread::ThreadId;

use crate::Scope;
use crate::{scope::Stack, Client, Hub};

static PROCESS_HUB: LazyLock<ProcessHub> = LazyLock::new(|| ProcessHub {
    hub: Arc::new(Hub::new(None, Arc::new(Default::default()))),
    thread: thread::current().id(),
});

thread_local! {
    /// The [`Hub`] associated with this thread.
    ///
    /// On the thread on which the [`PROCESS_HUB`] is initialized, the [`THREAD_HUB`] and
    /// [`PROCESS_HUB`] are identical, i.e. `Arc::ptr_eq(&PROCESS_HUB, &THREAD_HUB)` is true.
    /// On any other thread, the [`THREAD_HUB`] is created as a new hub based off of the
    /// [`PROCESS_HUB`].
    static THREAD_HUB: RefCell<Arc<Hub>> = if thread::current().id() == PROCESS_HUB.thread {
        PROCESS_HUB.hub.clone()
    } else {
        Hub::new_from_top(&PROCESS_HUB.hub).into()
    }.into()
}

/// A guard that temporarily swaps the active hub in thread-local storage.
///
/// This type is `!Send` because it manages thread-local state and must be
/// dropped on the same thread where it was created.
pub struct SwitchGuard {
    inner: Option<Arc<Hub>>,
    /// Makes this type `!Send` while keeping it `Sync`.
    ///
    /// ```rust
    /// # use sentry_core::HubSwitchGuard as SwitchGuard;
    /// trait AssertSync: Sync {}
    ///
    /// impl AssertSync for SwitchGuard {}
    /// ```
    ///
    /// ```rust,compile_fail
    /// # use sentry_core::HubSwitchGuard as SwitchGuard;
    /// trait AssertSend: Send {}
    ///
    /// impl AssertSend for SwitchGuard {}
    /// ```
    _not_send: PhantomData<MutexGuard<'static, ()>>,
}

impl SwitchGuard {
    /// Swaps the current thread's Hub by the one provided
    /// and returns a guard that, when dropped, replaces it
    /// to the previous one.
    pub fn new(mut hub: Arc<Hub>) -> Self {
        let inner = THREAD_HUB.with(|thread_hub| {
            let mut thread_hub = thread_hub.borrow_mut();
            if std::ptr::eq(thread_hub.as_ref(), hub.as_ref()) {
                return None;
            }
            std::mem::swap(&mut *thread_hub, &mut hub);
            Some(hub)
        });
        SwitchGuard {
            inner,
            _not_send: PhantomData,
        }
    }

    fn swap(&mut self) -> Option<Arc<Hub>> {
        self.inner.take().and_then(|mut hub| {
            // We use `try_with` to access the `THREAD_HUB`, intentionally ignoring any errors that
            // result. If `try_with` errors, this is because the `THREAD_HUB` local key has been
            // destroyed, which means that there is nothing to swap with, making this operation
            // pointless. Further, the destruction of the thread-local indicates that the thread
            // is likely shutting down.
            THREAD_HUB
                .try_with(|thread_hub| {
                    let mut thread_hub = thread_hub.borrow_mut();
                    std::mem::swap(&mut *thread_hub, &mut hub);
                    hub
                })
                .ok()
        })
    }
}

impl Drop for SwitchGuard {
    fn drop(&mut self) {
        let _ = self.swap();
    }
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

        guard.top().client.as_ref().is_some_and(|c| c.is_enabled())
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
        THREAD_HUB.with_borrow(Arc::clone)
    }

    /// Returns the main thread's hub.
    ///
    /// This is similar to [`Hub::current`] but instead of picking the
    /// current thread's hub it returns the main thread's hub instead.
    pub fn main() -> Arc<Hub> {
        PROCESS_HUB.hub.clone()
    }

    /// Invokes the callback with the default hub.
    #[deprecated = "Use `Hub::current` instead; this function offers no performance benefit."]
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Arc<Hub>) -> R,
    {
        f(&Hub::current())
    }

    /// Invokes the callback with a reference to the thread hub.
    ///
    /// This is potentially more performant than [`Hub::current`] as we avoid an [`Arc::clone`],
    /// but it holds a borrow to the [`THREAD_HUB`]'s `RefCell` for the duration of the callback.
    /// It is therefore essential to avoid calling [`Hub::run`], [`SwitchGuard::new`], or anything
    /// else that mutably borrows the [`THREAD_HUB`] during the callback, e.g. any user-supplied
    /// callbacks.
    ///
    /// # Panics
    /// Panics if the [`THREAD_HUB`] is mutably borrowed at any point during the callback.
    pub(crate) fn with_current<F, R>(f: F) -> R
    where
        F: FnOnce(&Hub) -> R,
    {
        THREAD_HUB.with_borrow(|hub| f(hub))
    }

    /// Binds a hub to the current thread for the duration of the call.
    ///
    /// During the execution of `f` the given hub will be installed as the
    /// thread-local hub.  So any call to [`Hub::current`] during this time
    /// will return the provided hub.
    ///
    /// Once the function is finished executing, including after it
    /// panicked, the original hub is re-installed if one was present.
    pub fn run<F: FnOnce() -> R, R>(hub: Arc<Hub>, f: F) -> R {
        let _guard = SwitchGuard::new(hub);
        f()
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

/// Helper struct for storing the [`PROCESS_HUB`].
struct ProcessHub {
    /// The process's main hub.
    hub: Arc<Hub>,
    /// The thread on which the main hub was initialized.
    thread: ThreadId,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for [`Hub::with`], ensuring that the `RefCell` borrow is not held during the callback.
    ///
    /// If we hold the `RefCell` borrow during the callback, this would panic.
    #[test]
    fn hub_run_inside_with_scope() {
        let outer_hub = Arc::new(Hub::new(None, Default::default()));
        let inner_hub = Arc::new(Hub::new(None, Default::default()));

        Hub::run(outer_hub, || {
            #[expect(deprecated)] // We are intentionally testing deprecated functionality
            Hub::with(|_| Hub::run(inner_hub, || {}));
        });
    }
}
