use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Stream;

use crate::Hub;

/// A stream that binds a `Hub` to its polling.
///
/// This activates the given hub for the duration of the inner stream's `poll_next`
/// method. Users usually do not need to construct this type manually, but
/// rather use the [`StreamExt::bind_hub`] method instead.
///
/// [`StreamExt::bind_hub`]: trait.StreamExt.html#method.bind_hub
#[derive(Debug)]
pub struct SentryStream<S> {
    hub: Arc<Hub>,
    stream: S,
}

impl<S> SentryStream<S> {
    /// Creates a new bound stream with a `Hub`.
    pub fn new(hub: Arc<Hub>, stream: S) -> Self {
        Self { hub, stream }
    }
}

impl<S> Stream for SentryStream<S>
where
    S: Stream,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let hub = self.hub.clone();
        // https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field
        let stream = unsafe { self.map_unchecked_mut(|s| &mut s.stream) };
        #[cfg(feature = "client")]
        {
            let _guard = crate::hub_impl::SwitchGuard::new(hub);
            stream.poll_next(cx)
        }
        #[cfg(not(feature = "client"))]
        {
            let _ = hub;
            stream.poll_next(cx)
        }
    }
}

/// Stream extensions for Sentry.
pub trait SentryStreamExt: Sized {
    /// Binds a hub to this stream.
    ///
    /// This ensures that the stream is polled within the given hub.
    fn bind_hub<H>(self, hub: H) -> SentryStream<Self>
    where
        H: Into<Arc<Hub>>,
    {
        SentryStream {
            stream: self,
            hub: hub.into(),
        }
    }
}

impl<S> SentryStreamExt for S where S: Stream {}

#[cfg(all(test, feature = "test"))]
mod tests {
    use crate::test::with_captured_events;
    use crate::{capture_error, capture_message, configure_scope, Hub, Level, SentryStreamExt};
    use futures::StreamExt;
    use tokio::runtime::Runtime;

    #[derive(Debug)]
    struct TestError(&'static str);

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for TestError {}

    #[test]
    fn test_streams() {
        let mut events = with_captured_events(|| {
            let runtime = Runtime::new().unwrap();

            // Two real streams, each bound to its own hub. The work inside each
            // stream runs during `poll_next`, so the captured errors must end up
            // tagged with the scope of the hub the stream was bound to.
            runtime.block_on(async {
                let stream1 = futures::stream::once(async {
                    configure_scope(|scope| scope.set_transaction(Some("transaction1")));
                    capture_error(&TestError("oh no from 1"));
                })
                .bind_hub(Hub::new_from_top(Hub::current()));

                let stream2 = futures::stream::once(async {
                    configure_scope(|scope| scope.set_transaction(Some("transaction2")));
                    capture_error(&TestError("oh no from 2"));
                })
                .bind_hub(Hub::new_from_top(Hub::current()));

                stream1.collect::<Vec<_>>().await;
                stream2.collect::<Vec<_>>().await;
            });

            capture_message("oh hai from outside", Level::Info);
        });

        events.sort_by(|a, b| a.transaction.cmp(&b.transaction));
        assert_eq!(events.len(), 3);

        // The message captured outside any bound stream has no transaction and no
        // exception, and sorts first.
        assert_eq!(events[0].transaction, None);
        assert!(events[0].exception.is_empty());

        // The errors captured inside `poll_next` carry the scope of their bound
        // hub and the expected exception payload.
        assert_eq!(events[1].transaction, Some("transaction1".into()));
        assert_eq!(events[1].exception[0].value, Some("oh no from 1".into()));
        assert_eq!(events[2].transaction, Some("transaction2".into()));
        assert_eq!(events[2].exception[0].value, Some("oh no from 2".into()));
    }
}
