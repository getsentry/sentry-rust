use crate::{ClientOptions, Event, Scope, Uuid};

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
}
