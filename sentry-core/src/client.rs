use crate::{ClientOptions, Event, Scope, Uuid};
use std::time::Duration;

/// TODO
#[derive(Clone, Debug)]
pub struct Client {
    pub(crate) options: ClientOptions,
}

impl Client {
    /// Captures an event and sends it to sentry.
    ///
    /// If a scope is given, it will be applied to the event, running all the
    /// registered event processors.
    pub fn capture_event(&self, event: Event<'static>, scope: Option<&Scope>) -> Option<Uuid> {
        let _ = (event, scope);
        todo!()
    }

    /// Flush all pending events.
    ///
    /// This returns `true` if the queue was successfully flushed in the
    /// given time or `false` if not (for instance because of a timeout).
    pub fn flush(&self, timeout: Duration) -> bool {
        let _ = timeout;
        todo!()
    }

    /// Close and drop the client, flushing all pending events.
    ///
    /// See [`Client::flush`] for more Information.
    pub fn close(self, timeout: Option<Duration>) -> bool {
        let _ = timeout;
        todo!()
    }
}
