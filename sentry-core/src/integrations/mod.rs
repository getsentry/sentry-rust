//! This module provides support for various integrations.
//!
//! Which integerations are available depends on the features that were compiled in.

use std::any::{type_name, Any};

use crate::protocol::Event;
use crate::ClientOptions;

/// Integration abstraction.
///
/// An Integration in sentry has two primary purposes.
/// It can act as an *Event Source*, which will capture new events;
/// or as an *Event Processor*, which can modify every `Event` flowing through
/// the pipeline.
// NOTE: we need `Any` here so that the `TypeId` machinery works correctly.
pub trait Integration: Sync + Send + Any + AsAny {
    /// Name of this integration.
    ///
    /// This will be added to the SDK information sent to sentry.
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }

    /// Called whenever the integration is attached to a Client.
    fn setup(&self, options: &mut ClientOptions) {
        let _ = options;
    }

    /// The Integrations Event Processor Hook.
    ///
    /// An integration can process, or even completely drop an `Event`.
    /// Examples include adding or processing a backtrace, obfuscate some
    /// personal information, or add additional information.
    fn process_event(&self, event: Event<'static>) -> Option<Event<'static>> {
        Some(event)
    }
}

// This is needed as a workaround to be able to safely downcast integrations
#[doc(hidden)]
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
