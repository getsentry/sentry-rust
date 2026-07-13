#![allow(dead_code)]

use std::any::Any;
use std::future::{self, Future};
use std::sync::mpsc;
use std::task::{Context, Poll, Waker};
use std::thread;

use sentry::protocol::{EnvelopeItem, Transaction};
use sentry::Envelope;
use tracing::Span;

/// Sets up `span_across_await`, then executes it such that the span gets
/// entered and exited from different threads.
pub fn futures_cross_thread_common(span: Span) -> Result<(), Box<dyn Any + Send + 'static>> {
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

/// Assert that the given envelopes contain exactly one `Envelope`, containing
/// exactly one `EnvelopeItem`, which is a `Transaction` with the given name.
pub fn assert_transaction(envelopes: Vec<Envelope>, name: &str) {
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

/// Enters a span and holds the returned guard across an `.await` boundary.
pub async fn span_across_await(span: Span) {
    let _entered = span.enter();
    yield_once().await;
    // `_entered` is dropped here, after the `.await` call.
}

/// Yields exactly once, then finishes.
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

/// Creates a new no-op `Context`.
pub fn noop_context() -> Context<'static> {
    Context::from_waker(Waker::noop())
}

/// Gets and asserts that there is exactly one item in the iterator.
fn get_and_assert_only_item<I>(item_iter: I, message: &str) -> I::Item
where
    I: IntoIterator,
{
    let mut iter = item_iter.into_iter();
    let item = iter.next().expect(message);
    assert!(iter.next().is_none(), "{message}");
    item
}
