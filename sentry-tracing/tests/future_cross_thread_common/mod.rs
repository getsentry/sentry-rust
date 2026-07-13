use std::any::Any;
use std::future::Future;
use std::sync::mpsc;
use std::thread;

use tracing::Span;

/// Sets up `span_across_await`, then executes it such that the span gets
/// entered and exited from different threads.
pub fn futures_cross_thread_common(span: Span) -> Result<(), Box<dyn Any + Send + 'static>> {
    let mut future = Box::pin(crate::future_span_common::span_across_await(span));

    let (tx, rx) = mpsc::channel();

    let thread1 = thread::spawn(move || {
        let poll = future
            .as_mut()
            .poll(&mut crate::future_span_common::noop_context());
        assert!(poll.is_pending(), "future should be pending");
        tx.send(future).expect("failed to send future");
    });

    let thread2 = thread::spawn(move || {
        let poll = rx
            .recv()
            .expect("failed to receive future")
            .as_mut()
            .poll(&mut crate::future_span_common::noop_context());

        assert!(poll.is_ready(), "future should be ready");
    });

    thread1
        .join()
        .expect("thread 1 panicked, but should have completed");

    thread2.join()
}
