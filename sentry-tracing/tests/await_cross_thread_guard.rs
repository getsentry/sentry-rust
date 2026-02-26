mod shared;

use std::future::{self, Future};
use std::sync::mpsc;
use std::task::{Context, Poll, Waker};
use std::thread;

use sentry::protocol::{EnvelopeItem, Transaction};
use sentry::Envelope;
use tracing::Level;

/// Test that the [`sentry_tracing::SentryLayer`]'s `on_exit` implementation panics
/// (only when `debug_assertions` are enabled) if a span is exited on a span that
/// was entered on a different thread than where it was exited. Here, we specifically
/// test a future that is awaited across threads, as that is probably the most common
/// scenario where this can occur.
#[test]
fn future_cross_thread() {
    let transport = shared::init_sentry(1.0);

    let mut future = Box::pin(span_across_await());

    let (tx, rx) = mpsc::channel();

    let thread1 = thread::spawn(move || {
        let poll = future.as_mut().poll(&mut noop_context());
        assert!(poll.is_pending(), "future should be pending");
        tx.send(future).expect("failed to send future");
    });

    let thread2 = thread::spawn(move || {
        let _ = rx
            .recv()
            .expect("failed to receive future")
            .as_mut()
            .poll(&mut noop_context());
    });

    thread1
        .join()
        .expect("thread 1 panicked, but should have completed");

    let thread2_panic_message = thread2
        .join()
        .expect_err("thread2 did not panic, but it should have")
        .downcast::<String>()
        .expect("expected thread2 to panic with a String message");

    assert!(
        thread2_panic_message.starts_with("[SentryLayer] missing HubSwitchGuard on exit for span"),
        "Thread 2's panicked, but not for the expected reason. It is also possible that the panic \
        message was changed without updating this test."
    );

    // Despite the panic, the transaction should still get sent to Sentry
    assert_transaction(transport.fetch_and_clear_envelopes());
}

/// Counterpart to [`future_cross_thread`]; here, we check that the panic asserted in
/// [`future_cross_thread`] is not triggered when the span is exited on the same thread
/// that it was entered on.
#[test]
fn futures_same_thread() {
    let transport = shared::init_sentry(1.0);

    let mut future = Box::pin(span_across_await());

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
    assert_transaction(transport.fetch_and_clear_envelopes());
}

/// Assert that the given envelopes contain exactly one [`Envelope`],
/// containing exactly one [`EnvelopeItem`], which is a [`Transaction`],
/// with [`name`](Transaction::name) `"span_across_await"`.
fn assert_transaction(envelopes: Vec<Envelope>) {
    let envelope = get_and_assert_only_item(envelopes, "expected exactly one envelope");
    let item = get_and_assert_only_item(envelope.into_items(), "expected exactly one item");

    assert!(
        matches!(
            item,
            EnvelopeItem::Transaction(Transaction {
                name: Some(expected_name),
                ..
            }) if expected_name == "span_across_await"
        ),
        "expected a Transaction item with name \"span_across_await\""
    );
}

/// A helper function which creates and [`enter`s](tracing::Span::enter)
/// a [`Span`](tracing::Span), holding the returned
/// [`Entered<'_>`](tracing::span::Entered) guard across an `.await` boundary.
async fn span_across_await() {
    let span = tracing::span!(Level::INFO, "span_across_await");
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
