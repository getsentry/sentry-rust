use std::future;
use std::task::{Context, Poll, Waker};

use tracing::Span;

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
