use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use crate::protocol::{Breadcrumb, Context, Event, Level, User, Value};
use crate::EventProcessor;

/// Holds contextual data for the current scope.
///
/// The scope is an object that can be cloned efficiently and stores data that
/// is locally relevant to an event. For instance, the scope will hold recorded
/// breadcrumbs and similar information.
///
/// The scope can be interacted with in two ways:
///
/// 1. The scope is routinely updated with information by functions such as
///    `add_breadcrumb` which will modify the currently top-most scope.
/// 2. The topmost scope can also be configured through the `configure_scope`
///    method.
///
/// Note that the scope can only be modified but not inspected. Only the
/// client can use the scope to extract information currently.
#[derive(Clone, Default)]
pub struct Scope {
    level: Option<Level>,
    fingerprint: Option<Arc<Vec<Cow<'static, str>>>>,
    transaction: Option<Arc<String>>,
    pub breadcrumbs: im::Vector<Breadcrumb>,
    user: Option<Arc<User>>,
    extra: im::HashMap<String, Value>,
    tags: im::HashMap<String, String>,
    contexts: im::HashMap<String, Context>,
    event_processors: im::Vector<Arc<EventProcessor>>,
}

impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scope")
            .field("level", &self.level)
            .field("fingerprint", &self.fingerprint)
            .field("transaction", &self.transaction)
            .field("breadcrumbs", &self.breadcrumbs)
            .field("user", &self.user)
            .field("extra", &self.extra)
            .field("tags", &self.tags)
            .field("contexts", &self.contexts)
            .field("event_processors", &self.event_processors.len())
            .finish()
    }
}

impl Scope {
    /// Clear the scope.
    ///
    /// By default a scope will inherit all values from the higher scope.
    /// In some situations this might not be what a user wants.  Calling
    /// this method will wipe all data contained within.
    pub fn clear(&mut self) {
        *self = Default::default();
    }

    /// Sets a level override.
    pub fn set_level(&mut self, level: Option<Level>) {
        self.level = level;
    }

    /// Sets the fingerprint.
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        self.fingerprint = fingerprint
            .map(|fp| Arc::new(fp.iter().map(|x| Cow::Owned((*x).to_string())).collect()))
    }

    /// Sets the transaction.
    pub fn set_transaction(&mut self, transaction: Option<&str>) {
        self.transaction = transaction.map(|txn| Arc::new(txn.to_string()));
    }

    /// Sets the user for the current scope.
    pub fn set_user(&mut self, user: Option<User>) {
        self.user = user.map(Arc::new);
    }

    /// Sets a tag to a specific value.
    #[allow(clippy::needless_pass_by_value)]
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        self.tags.insert(key.to_string(), value.to_string());
    }

    /// Removes a tag.
    pub fn remove_tag(&mut self, key: &str) {
        self.tags.remove(key);
    }

    /// Sets a context for a key.
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        self.contexts.insert(key.to_string(), value.into());
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        self.contexts.remove(key);
    }

    /// Sets a extra to a specific value.
    pub fn set_extra(&mut self, key: &str, value: Value) {
        self.extra.insert(key.to_string(), value);
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        self.extra.remove(key);
    }

    /// Add an event processor to the scope.
    pub fn add_event_processor(&mut self, f: EventProcessor) {
        self.event_processors.push_back(Arc::new(f));
    }

    /// Applies the contained scoped data to fill an event.
    pub fn apply_to_event(&self, mut event: Event<'static>) -> Option<Event<'static>> {
        // TODO: event really should have an optional level
        if let Some(level) = self.level {
            event.level = level;
        }

        event.breadcrumbs.extend(self.breadcrumbs.iter().cloned());
        event.extra.extend(self.extra.iter().cloned());
        event.tags.extend(self.tags.iter().cloned());

        if event.user.is_none() {
            if let Some(ref user) = self.user {
                event.user = Some((**user).clone());
            }
        }

        event.contexts.extend(self.contexts.iter().cloned());

        if event.transaction.is_none() {
            if let Some(ref txn) = self.transaction {
                event.transaction = Some((**txn).clone());
            }
        }

        if event.fingerprint.len() == 1
            && (event.fingerprint[0] == "{{ default }}" || event.fingerprint[0] == "{{default}}")
        {
            if let Some(ref fp) = self.fingerprint {
                event.fingerprint = Cow::Owned((**fp).clone());
            }
        }

        for processor in &self.event_processors {
            event = processor(event)?;
        }

        Some(event)
    }
}
