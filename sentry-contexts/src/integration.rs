use std::borrow::Cow;

use sentry_core::protocol::map::Entry;
use sentry_core::protocol::Event;
use sentry_core::{ClientOptions, Integration};

use crate::utils::{device_context, os_context, rust_context, server_name};

/// Adds Contexts to Sentry Events.
///
/// See the [Contexts Interface] documentation for more info.
///
/// [Contexts Interface]: https://develop.sentry.dev/sdk/event-payloads/contexts/
pub struct ContextIntegration {
    /// Add `os` context, enabled by default.
    pub add_os: bool,
    /// Add `rust` context, enabled by default.
    pub add_rust: bool,
    /// Add `device` context, enabled by default.
    pub add_device: bool,
}

impl Default for ContextIntegration {
    fn default() -> Self {
        Self {
            add_os: true,
            add_rust: true,
            add_device: true,
        }
    }
}

impl ContextIntegration {
    /// Create a new Context Integration.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Integration for ContextIntegration {
    fn name(&self) -> &'static str {
        "contexts"
    }

    fn setup(&self, options: &mut ClientOptions) {
        if options.server_name.is_none() {
            options.server_name = server_name().map(Cow::Owned);
        }
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        _cfg: &ClientOptions,
    ) -> Option<Event<'static>> {
        if self.add_os {
            if let Entry::Vacant(entry) = event.contexts.entry("os".to_string()) {
                if let Some(os) = os_context() {
                    entry.insert(os);
                }
            }
        }
        if self.add_rust {
            event
                .contexts
                .entry("rust".to_string())
                .or_insert_with(rust_context);
        }
        if self.add_device {
            event
                .contexts
                .entry("device".to_string())
                .or_insert_with(device_context);
        }

        Some(event)
    }
}
