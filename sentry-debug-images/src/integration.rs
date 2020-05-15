use std::borrow::Cow;

use sentry_core::protocol::{DebugMeta, Event};
use sentry_core::{ClientOptions, Integration};

/// The Sentry Debug Images Integration.
pub struct DebugImagesIntegration {
    /// A custom filter for which Events should get debug images.
    pub filter: Box<dyn Fn(&Event<'static>) -> bool + Send + Sync>,
}

impl DebugImagesIntegration {
    /// Creates a new Debug Images Integration.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for DebugImagesIntegration {
    fn default() -> Self {
        Self {
            filter: Box::new(|_| true),
        }
    }
}

impl Integration for DebugImagesIntegration {
    fn name(&self) -> &'static str {
        "debug-images"
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        _opts: &ClientOptions,
    ) -> Option<Event<'static>> {
        lazy_static::lazy_static! {
            static ref DEBUG_META: DebugMeta = DebugMeta {
                images: crate::debug_images(),
                ..Default::default()
            };
        }

        if event.debug_meta.is_empty() && (self.filter)(&event) {
            event.debug_meta = Cow::Borrowed(&DEBUG_META);
        }

        Some(event)
    }
}
