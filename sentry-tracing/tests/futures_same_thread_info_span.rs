mod future_span_common;
mod shared;

use std::future::Future;
use std::thread;

use sentry::{Hub, HubSwitchGuard};

/// Tests that the panic asserted in `future_cross_thread_info_span` is not
/// triggered when the span is exited on the same thread that it was entered
/// on.
#[test]
fn futures_same_thread_info_span() {
    let _guard = HubSwitchGuard::new(Hub::new_from_top(Hub::current()).into());
    let transport = shared::init_sentry(1.0);

    let span = tracing::info_span!("futures_same_thread_info_span");
    let mut future = Box::pin(future_span_common::span_across_await(span));

    let thread = thread::spawn(move || {
        assert!(
            future
                .as_mut()
                .poll(&mut future_span_common::noop_context())
                .is_pending(),
            "first poll should be pending"
        );
        assert!(
            future
                .as_mut()
                .poll(&mut future_span_common::noop_context())
                .is_ready(),
            "second poll should be ready"
        );
    });

    thread.join().expect("thread should complete successfully");
    future_span_common::assert_transaction(
        transport.fetch_and_clear_envelopes(),
        "futures_same_thread_info_span",
    );
}
