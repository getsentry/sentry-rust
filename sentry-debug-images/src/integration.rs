use std::borrow::Cow;
use std::sync::LazyLock;

use sentry_core::protocol::{DebugMeta, Event};
use sentry_core::{ClientOptions, Integration, sentry_debug};

static DEBUG_META: LazyLock<DebugMeta> = LazyLock::new(|| {
    sentry_debug!("[DebugImagesIntegration] Loading debug images");
    let debug_meta = DebugMeta {
        images: crate::debug_images(),
        ..Default::default()
    };
    sentry_debug!("[DebugImagesIntegration] Loaded {} debug images", debug_meta.images.len());
    debug_meta
});

/// The Sentry Debug Images Integration.
pub struct DebugImagesIntegration {
    filter: Box<dyn Fn(&Event<'_>) -> bool + Send + Sync>,
}

impl DebugImagesIntegration {
    /// Creates a new Debug Images Integration.
    pub fn new() -> Self {
        sentry_debug!("[DebugImagesIntegration] Creating new debug images integration");
        Self::default()
    }

    /// Sets a custom filter function.
    ///
    /// The filter specified which [`Event`]s should get debug images.
    #[must_use]
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Event<'_>) -> bool + Send + Sync + 'static,
    {
        sentry_debug!("[DebugImagesIntegration] Setting custom filter function");
        self.filter = Box::new(filter);
        self
    }
}

impl Default for DebugImagesIntegration {
    fn default() -> Self {
        sentry_debug!("[DebugImagesIntegration] Creating default debug images integration");
        LazyLock::force(&DEBUG_META);
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
        if event.debug_meta.is_empty() && (self.filter)(&event) {
            sentry_debug!("[DebugImagesIntegration] Adding debug images to event {}", event.event_id);
            event.debug_meta = Cow::Borrowed(&DEBUG_META);
            sentry_debug!("[DebugImagesIntegration] Added {} debug images to event {}", 
                         DEBUG_META.images.len(), event.event_id);
        } else if !event.debug_meta.is_empty() {
            sentry_debug!("[DebugImagesIntegration] Event {} already has debug metadata", event.event_id);
        } else {
            sentry_debug!("[DebugImagesIntegration] Filter rejected event {} for debug images", event.event_id);
        }

        Some(event)
    }
}
