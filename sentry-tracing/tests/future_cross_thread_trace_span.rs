mod future_cross_thread_common;
mod future_span_common;
mod shared;

use sentry::{Hub, HubSwitchGuard};

/// Tests that no panic occurs when a future containing a span that is not
/// captured by Sentry is awaited across threads.
#[test]
fn future_cross_thread_trace_span() {
    let _guard = HubSwitchGuard::new(Hub::new_from_top(Hub::current()).into());
    let transport = shared::init_sentry(1.0);

    let span = tracing::trace_span!("future_cross_thread_trace_span");

    future_cross_thread_common::futures_cross_thread_common(span)
        .expect("no panic should occur for spans not captured by Sentry");

    assert!(
        transport.fetch_and_clear_envelopes().is_empty(),
        "No envelopes should be sent for spans not captured by Sentry"
    );
}
