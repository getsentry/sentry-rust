mod future_span_common;
mod shared;

use sentry::{Hub, HubSwitchGuard};

/// Tests that `SentryLayer`'s `on_exit` implementation panics (only when
/// `debug_assertions` are enabled) if a Sentry-captured span is exited on a
/// different thread than where it was entered.
///
/// This specifically tests a future awaited across threads, which is probably
/// the most common scenario where this can occur. The span is at info level,
/// the lowest level captured by Sentry by default.
#[test]
fn future_cross_thread_info_span() {
    const SPAN_NAME: &str = "future_cross_thread_info_span";

    let _guard = HubSwitchGuard::new(Hub::new_from_top(Hub::current()).into());
    let transport = shared::init_sentry(1.0);

    let span = tracing::info_span!(SPAN_NAME);

    let thread2_result = future_span_common::futures_cross_thread_common(span);

    // Panic should only occur when debug_assertions are enabled.
    #[cfg(debug_assertions)]
    {
        let thread2_panic_message = thread2_result
            .expect_err("thread2 did not panic, but it should have (debug_assertions enabled)")
            .downcast::<String>()
            .expect("expected thread2 to panic with a String message");

        assert!(
            thread2_panic_message.starts_with(
                "[SentryLayer] missing HubSwitchGuard on exit for span"
            ),
            "Thread 2 panicked, but not for the expected reason. It is also possible that the panic \
            message was changed without updating this test."
        );
    }

    #[cfg(not(debug_assertions))]
    thread2_result.expect("thread2 should not panic if debug_assertions are disabled");

    future_span_common::assert_transaction(transport.fetch_and_clear_envelopes(), SPAN_NAME);
}
