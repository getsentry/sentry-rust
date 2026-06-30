mod shared;

use std::any::Any;
use std::future::{self, Future};
use std::sync::mpsc;
use std::task::{Context, Poll, Waker};
use std::thread;

use sentry::protocol::{EnvelopeItem, Transaction};
use sentry::{Envelope, Hub, HubSwitchGuard};
use tracing::Span;

/// Test that the [`sentry_tracing::SentryLayer`]'s `on_exit` implementation panics
/// (only when `debug_assertions` are enabled) if a span, captured by Sentry, is exited
/// on a different thread than where it was entered.
///
/// Here, we specifically test a future that is awaited across threads, as that is
/// probably the most common scenario where this can occur.
///
/// We use an info-level span, as that's the lowest level captured by Sentry by default.
#[test]
fn future_cross_thread_info_span() {
    const SPAN_NAME: &str = "future_cross_thread_info_span";

    let _guard = HubSwitchGuard::new(Hub::new_from_top(Hub::current()).into());
    let transport = shared::init_sentry(1.0);

    let span = tracing::info_span!(SPAN_NAME);

    let thread2_result = futures_cross_thread_common(span);

    // Panic should only occur when debug_assertions are enabled.
    #[cfg(debug_assertions)]
    {
        let thread2_panic_message = thread2_result
            .expect_err("thread2 did not panic, but it should have (debug_assertions enabled)")
            .downcast::<String>()
            .expect("expected thread2 to panic with a String message");

        assert!(
            thread2_panic_message.starts_with("[SentryLayer] missing HubSwitchGuard on exit for span"),
            "Thread 2 panicked, but not for the expected reason. It is also possible that the panic \
            message was changed without updating this test."
        );
    }

    #[cfg(not(debug_assertions))]
    thread2_result.expect("thread2 should not panic if debug_assertions are disabled");

    assert_transaction(transport.fetch_and_clear_envelopes(), SPAN_NAME);
}

/// Counterpart to [`future_cross_thread_info_span`]; here, we check that no panic occurs
/// if the span is not captured by Sentry.
///
/// No panic should occur because we do not change the [`Hub`] for spans that we don't capture,
/// and so, we should not expect to pop a [`HubSwitchGuard`].
#[test]
fn future_cross_thread_trace_span() {
    let _guard = HubSwitchGuard::new(Hub::new_from_top(Hub::current()).into());
    let transport = shared::init_sentry(1.0);

    let span = tracing::trace_span!("future_cross_thread_trace_span");

    futures_cross_thread_common(span)
        .expect("no panic should occur for spans not captured by Sentry");

    assert!(
        transport.fetch_and_clear_envelopes().is_empty(),
        "No envelopes should be sent for spans not captured by Sentry"
    );
}

/// Counterpart to [`future_cross_thread_info_span`]; here, we check that the panic asserted in
/// [`future_cross_thread`] is not triggered when the span is exited on the same thread
/// that it was entered on.
#[test]
fn futures_same_thread_info_span() {
    let _guard = HubSwitchGuard::new(Hub::new_from_top(Hub::current()).into());
    let transport = shared::init_sentry(1.0);

    let span = tracing::info_span!("futures_same_thread_info_span");
    let mut future = Box::pin(span_across_await(span));

    let thread = thread::spawn(move || {
        assert!(
            future.as_mut().poll(&mut noop_context()).is_pending(),
            "first poll should be pending"
        );
        assert!(
            future.as_mut().poll(&mut noop_context()).is_ready(),
            "second poll should be ready"
        );
    });

    thread.join().expect("thread should complete successfully");
    assert_transaction(
        transport.fetch_and_clear_envelopes(),
        "futures_same_thread_info_span",
    );
}

/// Common logic for cross-thread tests.
///
/// This function sets up the [`span_across_await`] future, then executes it such that
/// the span gets entered and exited from different threads.
fn futures_cross_thread_common(span: Span) -> Result<(), Box<dyn Any + Send + 'static>> {
    let mut future = Box::pin(span_across_await(span));

    let (tx, rx) = mpsc::channel();

    let thread1 = thread::spawn(move || {
        let poll = future.as_mut().poll(&mut noop_context());
        assert!(poll.is_pending(), "future should be pending");
        tx.send(future).expect("failed to send future");
    });

    let thread2 = thread::spawn(move || {
        let poll = rx
            .recv()
            .expect("failed to receive future")
            .as_mut()
            .poll(&mut noop_context());

        assert!(poll.is_ready(), "future should be ready");
    });

    thread1
        .join()
        .expect("thread 1 panicked, but should have completed");

    thread2.join()
}

/// Assert that the given envelopes contain exactly one [`Envelope`],
/// containing exactly one [`EnvelopeItem`], which is a [`Transaction`],
/// with given [`name`](Transaction::name).
fn assert_transaction(envelopes: Vec<Envelope>, name: &str) {
    let envelope = get_and_assert_only_item(envelopes, "expected exactly one envelope");
    let item = get_and_assert_only_item(envelope.into_items(), "expected exactly one item");

    assert!(
        matches!(
            item,
            EnvelopeItem::Transaction(Transaction {
                name: Some(expected_name),
                ..
            }) if expected_name == name
        ),
        "expected a Transaction item with name {name:?}"
    );
}

/// A helper function which and [`enter`s](tracing::Span::enter)
/// a given [`Span`](tracing::Span), holding the returned
/// [`Entered<'_>`](tracing::span::Entered) guard across an `.await` boundary.
async fn span_across_await(span: Span) {
    let _entered = span.enter();
    yield_once().await;
    // _entered dropped here, after .await call
}

/// Helper function that yields exactly once, then finishes.
async fn yield_once() {
    let mut yielded = false;

    future::poll_fn(|_| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            Poll::Pending
        }
    })
    .await;
}

/// Helper function to create a new no-op [`Context`].
fn noop_context() -> Context<'static> {
    Context::from_waker(Waker::noop())
}

/// Helper function to get and assert that there is exactly one item in the iterator.
/// Extracts the only item from the iterator and returns it, or panics with the
/// provided message if there are zero or multiple items.
fn get_and_assert_only_item<I>(item_iter: I, message: &str) -> I::Item
where
    I: IntoIterator,
{
    let mut iter = item_iter.into_iter();
    let item = iter.next().expect(message);
    assert!(iter.next().is_none(), "{message}");
    item
}
