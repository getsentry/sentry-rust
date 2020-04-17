use crate::Event;

/// A generic Event Processor
///
/// The Event Processor is invoked during different stages of the pipeline.
/// It can add more information to an event, modify existing information, or
/// decide to discard the event altogether, in which case further processing and
/// uploading is skipped.
pub trait EventProcessor: Send + Sync {
    /// Processes an event.
    fn process_event(&self, event: Event<'static>) -> Option<Event<'static>> {
        Some(event)
    }
}

impl<F> EventProcessor for F
where
    F: Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync,
{
    fn process_event(&self, event: Event<'static>) -> Option<Event<'static>> {
        self(event)
    }
}
