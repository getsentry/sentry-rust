use std::borrow::Cow;

use sentry_core::protocol::map::Entry;
use sentry_core::protocol::Event;
use sentry_core::{ClientOptions, Integration, sentry_debug};

use crate::utils::{device_context, os_context, rust_context, server_name};

/// Adds Contexts to Sentry Events.
///
/// This integration is enabled by default in `sentry` and adds `device`, `os`
/// and `rust` contexts to Events, and also sets a `server_name` if it is not
/// already defined.
///
/// See the [Contexts Interface] documentation for more info.
///
/// # Examples
///
/// ```rust
/// let integration = sentry_contexts::ContextIntegration::new().add_os(false);
/// let _sentry = sentry::init(sentry::ClientOptions::new().add_integration(integration));
/// ```
///
/// [Contexts Interface]: https://develop.sentry.dev/sdk/event-payloads/contexts/
#[derive(Debug)]
pub struct ContextIntegration {
    add_os: bool,
    add_rust: bool,
    add_device: bool,
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

    /// Add `os` context, enabled by default.
    #[must_use]
    pub fn add_os(mut self, add_os: bool) -> Self {
        self.add_os = add_os;
        self
    }
    /// Add `rust` context, enabled by default.
    #[must_use]
    pub fn add_rust(mut self, add_rust: bool) -> Self {
        self.add_rust = add_rust;
        self
    }

    /// Add `device` context, enabled by default.
    #[must_use]
    pub fn add_device(mut self, add_device: bool) -> Self {
        self.add_device = add_device;
        self
    }
}

impl Integration for ContextIntegration {
    fn name(&self) -> &'static str {
        "contexts"
    }

    fn setup(&self, options: &mut ClientOptions) {
        sentry_debug!("[ContextIntegration] Setting up contexts integration");
        if options.server_name.is_none() {
            if let Some(server_name) = server_name() {
                sentry_debug!("[ContextIntegration] Setting server_name from system: {}", server_name);
                options.server_name = Some(Cow::Owned(server_name));
            }
        }
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        _cfg: &ClientOptions,
    ) -> Option<Event<'static>> {
        sentry_debug!("[ContextIntegration] Processing event {}", event.event_id);
        
        let mut contexts_added = Vec::new();
        
        if self.add_os {
            if let Entry::Vacant(entry) = event.contexts.entry("os".to_string()) {
                if let Some(os) = os_context() {
                    entry.insert(os);
                    contexts_added.push("os");
                }
            }
        }
        if self.add_rust {
            event
                .contexts
                .entry("rust".to_string())
                .or_insert_with(rust_context);
            contexts_added.push("rust");
        }
        if self.add_device {
            event
                .contexts
                .entry("device".to_string())
                .or_insert_with(device_context);
            contexts_added.push("device");
        }

        if !contexts_added.is_empty() {
            sentry_debug!("[ContextIntegration] Added contexts: {}", contexts_added.join(", "));
        }

        Some(event)
    }
}
