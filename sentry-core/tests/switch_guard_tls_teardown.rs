//! Regression tests for [issue #1237].
//!
//! The tests cover both possible destruction orders for the thread-local
//! storage containing `HubSwitchGuard` values and `THREAD_HUB`. TLS destruction
//! order is not guaranteed, so testing both orders increases the likelihood
//! that a reintroduced bug is detected on a given platform.
//!
//! [issue #1237]: https://github.com/getsentry/sentry-rust/issues/1237

use std::cell::RefCell;
use std::sync::Arc;
use std::thread;

use sentry::{Hub, HubSwitchGuard};

/// Initializes the guard TLS before `THREAD_HUB`, reliably reproducing the
/// reported failure on macOS targets. The test passes once the bug is fixed.
#[test]
fn switch_guard_tolerates_destroyed_thread_hub() {
    thread_local! {
        static STORED_GUARD: RefCell<Option<HubSwitchGuard>> =
            const { RefCell::new(None) };
    }

    thread::spawn(|| {
        // Initialize this TLS value before THREAD_HUB.
        STORED_GUARD.with(|_| {});

        // Hub::current initializes THREAD_HUB.
        let current = Hub::current();
        let replacement = Arc::new(Hub::new_from_top(current));
        let guard = HubSwitchGuard::new(replacement);

        // Keep the guard alive until TLS teardown.
        STORED_GUARD.with_borrow_mut(|slot| {
            *slot = Some(guard);
        });
    })
    .join()
    .expect("worker should exit without panicking");
}

/// Initializes the guard TLS after `THREAD_HUB` and covers the other possible
/// TLS destruction order. TLS destruction order is not guaranteed, so this test
/// helps detect a reintroduced bug on platforms where the first order does not
/// reliably occur.
#[test]
fn switch_guard_drops_before_thread_hub() {
    thread_local! {
        static STORED_GUARD: RefCell<Option<HubSwitchGuard>> =
            const { RefCell::new(None) };
    }

    thread::spawn(|| {
        // Hub::current initializes THREAD_HUB before STORED_GUARD.
        let current = Hub::current();
        let replacement = Arc::new(Hub::new_from_top(current));
        let guard = HubSwitchGuard::new(replacement);

        // Keep the guard alive until TLS teardown, but initialize its TLS
        // after THREAD_HUB.
        STORED_GUARD.with_borrow_mut(|slot| {
            *slot = Some(guard);
        });
    })
    .join()
    .expect("worker should exit without panicking");
}
