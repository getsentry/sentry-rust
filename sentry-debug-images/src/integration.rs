use std::borrow::Cow;

use sentry_core::protocol::{DebugMeta, Event};
use sentry_core::{ClientOptions, Integration};

/// The Sentry Debug Images Integration.
pub struct DebugImagesIntegration {
    filter: Box<dyn Fn(&Event<'_>) -> bool + Send + Sync>,
}

impl DebugImagesIntegration {
    /// Creates a new Debug Images Integration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a custom filter function.
    ///
    /// The filter specified which [`Event`]s should get debug images.
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Event<'_>) -> bool + Send + Sync + 'static,
    {
        self.filter = Box::new(filter);
        self
    }
}

impl Default for DebugImagesIntegration {
    fn default() -> Self {
        Self {
            filter: Box::new(|_| true),
        }
    }
}

impl std::fmt::Debug for DebugImagesIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        struct Filter;
        f.debug_struct("DebugImagesIntegration")
            .field("filter", &Filter)
            .finish()
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
