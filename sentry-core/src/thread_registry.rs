//! Maps operating system thread ids to the hub that is current on each
//! thread.
//!
//! Out-of-process crash reporters learn the crashing thread from the OS,
//! not from Rust. This registry lets another thread in the same process
//! find the [`Hub`] that thread was using, so its scope can be reported
//! with the crash.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError, RwLock, Weak};

use crate::Hub;

/// Returns the id the operating system uses for the current thread.
///
/// This is the id a crash handler sees for the crashing thread:
///
/// - Linux and Android: the kernel thread id from `gettid`.
/// - Windows: `GetCurrentThreadId`.
/// - macOS: the Mach port name of the thread, as reported in exception
///   messages and returned by `pthread_mach_thread_np`.
///
/// Returns `None` when the OS refuses the call, for example when a
/// seccomp filter blocks `gettid`.
///
/// It is not related to [`std::thread::ThreadId`].
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows
))]
pub fn current_os_thread_id() -> Option<u64> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // SAFETY: `gettid` takes no arguments.
        thread_id_from_syscall(unsafe { libc::syscall(libc::SYS_gettid) })
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: `pthread_self` is always valid for the calling thread.
        Some(unsafe { libc::pthread_mach_thread_np(libc::pthread_self()) }.into())
    }
    #[cfg(windows)]
    {
        extern "system" {
            fn GetCurrentThreadId() -> u32;
        }
        // SAFETY: takes no arguments and cannot fail.
        Some(unsafe { GetCurrentThreadId() }.into())
    }
}

/// Turns the result of the `gettid` syscall into a thread id.
///
/// A seccomp filter can make the syscall return -1 without running it.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn thread_id_from_syscall(ret: libc::c_long) -> Option<u64> {
    u64::try_from(ret).ok().filter(|&id| id > 0)
}

/// The id to register the current thread under, if the OS reports one.
fn registry_id() -> Option<u64> {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        windows
    ))]
    {
        current_os_thread_id()
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        windows
    )))]
    {
        None
    }
}

/// One thread's entry in the registry.
struct Slot {
    /// `None` when the thread has no OS id and is not in the registry.
    os_thread_id: Option<u64>,
    hub: RwLock<Weak<Hub>>,
}

static REGISTRY: LazyLock<Mutex<HashMap<u64, Arc<Slot>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Owns a thread's slot and removes it when the thread exits.
struct SlotGuard(Arc<Slot>);

impl SlotGuard {
    fn register(os_thread_id: Option<u64>) -> Self {
        let slot = Arc::new(Slot {
            os_thread_id,
            hub: RwLock::new(Weak::new()),
        });
        if let Some(id) = os_thread_id {
            REGISTRY
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(id, slot.clone());
        }
        SlotGuard(slot)
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        let Some(id) = self.0.os_thread_id else {
            return;
        };
        let mut registry = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
        // The OS can reuse the id for a new thread before this one is
        // fully gone, so only remove the entry if it is still ours.
        if registry
            .get(&id)
            .is_some_and(|slot| Arc::ptr_eq(slot, &self.0))
        {
            registry.remove(&id);
        }
    }
}

thread_local! {
    static SLOT: SlotGuard = SlotGuard::register(registry_id());
}

/// Records `hub` as the hub current on the calling thread.
///
/// Does nothing while the thread is shutting down and its slot is gone.
pub(crate) fn set_current_hub(hub: &Arc<Hub>) {
    let _ = SLOT.try_with(|slot| {
        *slot.0.hub.write().unwrap_or_else(PoisonError::into_inner) = Arc::downgrade(hub);
    });
}

/// Returns the hub current on the thread with the given OS id.
pub(crate) fn hub_for_os_thread(os_thread_id: u64) -> Option<Arc<Hub>> {
    let slot = REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&os_thread_id)
        .cloned()?;
    let hub = slot.hub.read().unwrap_or_else(PoisonError::into_inner);
    hub.upgrade()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn registers_current_hub_per_thread() {
        let (id_tx, id_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel::<()>();

        let worker = std::thread::spawn(move || {
            let hub = Hub::current();
            id_tx.send((current_os_thread_id().unwrap(), hub)).unwrap();
            // Keep the thread alive until the main thread has looked it up.
            done_rx.recv().ok();
        });

        let (id, hub) = id_rx.recv().unwrap();
        let found = Hub::for_os_thread(id).expect("worker thread is registered");
        assert!(Arc::ptr_eq(&found, &hub));
        assert!(!Arc::ptr_eq(&found, &Hub::main()));

        done_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(Hub::for_os_thread(id).is_none(), "slot is removed on exit");
    }

    #[test]
    fn follows_hub_run_switching() {
        let id = current_os_thread_id().unwrap();
        let outer = Hub::current();
        let inner = Arc::new(Hub::new_from_top(&outer));

        Hub::run(inner.clone(), || {
            let found = Hub::for_os_thread(id).unwrap();
            assert!(Arc::ptr_eq(&found, &inner));
        });

        let found = Hub::for_os_thread(id).unwrap();
        assert!(Arc::ptr_eq(&found, &outer));
    }

    #[test]
    fn unknown_thread_has_no_hub() {
        assert!(Hub::for_os_thread(u64::MAX).is_none());
    }

    #[test]
    fn thread_without_id_is_not_registered() {
        let guard = SlotGuard::register(None);
        assert!(!REGISTRY
            .lock()
            .unwrap()
            .values()
            .any(|slot| Arc::ptr_eq(slot, &guard.0)));
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn failed_gettid_has_no_id() {
        assert_eq!(thread_id_from_syscall(-1), None);
        assert_eq!(thread_id_from_syscall(0), None);
        assert_eq!(thread_id_from_syscall(42), Some(42));
    }

    #[test]
    fn scope_snapshot_works_while_the_scope_is_read() {
        let hub = Hub::new(None, Default::default());
        hub.configure_scope(|scope| scope.set_tag("thread", "worker"));

        // A write lock here would never be granted.
        let snapshot = hub.with_current_scope(|_| hub.scope_snapshot());
        let event = snapshot
            .apply_to_event(Default::default())
            .expect("no event processors drop the event");
        assert_eq!(event.tags.get("thread").map(String::as_str), Some("worker"));
    }
}
