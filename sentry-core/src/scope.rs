use std::{borrow::Cow, fmt, sync::Arc};

use crate::protocol::{Context, Value};
use crate::{Breadcrumb, Event, EventProcessor, Level, User};

/// TODO
pub struct ScopeGuard {}

/// Holds contextual data for the current scope.
///
/// The scope is an object that can be cloned efficiently and stores data that
/// is locally relevant to an event. For instance the scope will hold recorded
/// breadcrumbs and similar information. More about it can be found in the
/// [Unified API](https://docs.sentry.io/development/sdk-dev/unified-api/#scope)
/// document.
///
/// The scope can be interacted with in two ways:
///
/// 1. The scope is routinely updated with information by functions such as
///    `add_breadcrumb` which will modify the currently top-most scope.
/// 2. The topmost scope can also be configured through the `configure_scope`
///    method.
///
/// Note that the scope can only be modified but not inspected.  Only the
/// client can use the scope to extract information currently.
#[derive(Clone, Default)]
pub struct Scope {
    pub(crate) level: Option<Level>,
    pub(crate) fingerprint: Option<Arc<Vec<Cow<'static, str>>>>,
    pub(crate) transaction: Option<Arc<String>>,
    pub(crate) breadcrumbs: im::Vector<Breadcrumb>,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) extra: im::HashMap<String, Value>,
    pub(crate) tags: im::HashMap<String, String>,
    pub(crate) contexts: im::HashMap<String, Context>,
    pub(crate) event_processors: im::Vector<Arc<dyn EventProcessor>>,
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
    /// Creates a new empty scope.
    pub fn new() -> Self {
        Default::default()
    }

    /// Clear the scope.
    ///
    /// By default a scope will inherit all values from the higher scope.
    /// In some situations this might not be what a user wants. Calling
    /// this method will wipe all data contained within. However, all registered
    /// Event Processors will be kept.
    pub fn clear(&mut self) {
        let event_processors = self.event_processors.clone();
        *self = Scope {
            event_processors,
            ..Default::default()
        };
    }

    /// Sets a level override.
    ///
    /// This overrides the level of every event captured inside this scope.
    pub fn set_level(&mut self, level: Option<Level>) {
        self.level = level;
    }

    /// Sets the fingerprint to group specific events together.
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        self.fingerprint = fingerprint
            .map(|fp| Arc::new(fp.iter().map(|x| Cow::Owned((*x).to_string())).collect()));
    }

    /// Sets the name of the current transaction.
    pub fn set_transaction(&mut self, transaction: Option<&str>) {
        self.transaction = transaction.map(|txn| Arc::new(txn.to_string()));
    }

    /// Sets the user for the current scope.
    pub fn set_user(&mut self, user: Option<User>) {
        self.user = user.map(Arc::new);
    }

    /// Sets a tag to a specific value.
    ///
    /// Tags are arbitrary string values that can be used for issue
    /// categorization.
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        self.tags.insert(key.to_string(), value.to_string());
    }

    /// Removes a tag.
    pub fn remove_tag(&mut self, key: &str) {
        self.tags.remove(key);
    }

    /// Sets a context for a key.
    ///
    /// The context describes the runtime environment, such as OS, Device and
    /// other information.
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        self.contexts.insert(key.to_string(), value.into());
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        self.contexts.remove(key);
    }

    /// Sets a extra to a specific value.
    ///
    /// An extra is free-form JSON data that will be saved along the event.
    pub fn set_extra(&mut self, key: &str, value: Value) {
        self.extra.insert(key.to_string(), value);
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        self.extra.remove(key);
    }

    /// Add an event processor to the scope.
    ///
    /// The event processors will be executed in order when `apply_to_event` is
    /// called. They can be used to apply event specific data relevant to this
    /// scope, which is not part of this `Scope` data.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry_core::{Event, Level, Scope};
    ///
    /// let mut scope = Scope::new();
    /// scope.add_event_processor(|mut event: Event<'static>| {
    ///     event.level = Level::Error;
    ///     Some(event)
    /// });
    ///
    /// let event = scope.apply_to_event(Default::default()).unwrap();
    ///
    /// assert_eq!(event.level, Level::Error);
    /// ```
    pub fn add_event_processor<E>(&mut self, event_processor: E)
    where
        E: EventProcessor + 'static,
    {
        self.event_processors.push_back(Arc::new(event_processor));
    }

    /// Applies the contained scoped data to fill an event.
    ///
    /// Event Processors are called in order as part of this function. They
    /// might discard the Event altogether.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry_core::{Event, Level, Scope};
    ///
    /// let event = Event {
    ///     level: Level::Info,
    ///     transaction: Some("explicit transaction".into()),
    ///     ..Default::default()
    /// };
    /// let mut scope = Scope::new();
    /// scope.set_level(Some(Level::Warning));
    /// scope.set_tag("foo", "bar");
    /// scope.set_transaction(Some("example".into()));
    ///
    /// let event = scope.apply_to_event(event).unwrap();
    ///
    /// assert_eq!(event.level, Level::Warning);
    /// assert_eq!(event.tags, {
    ///     let mut map = sentry_core::protocol::Map::new();
    ///     map.insert("foo".to_string(), "bar".to_string());
    ///     map
    /// });
    /// assert_eq!(event.transaction.unwrap(), "explicit transaction".to_string());
    /// ```
    pub fn apply_to_event(&self, mut event: Event<'static>) -> Option<Event<'static>> {
        // TODO: event really should have an optional level
        if self.level.is_some() {
            event.level = self.level.unwrap();
        }

        if event.user.is_none() {
            if let Some(ref user) = self.user {
                event.user = Some((**user).clone());
            }
        }

        event.breadcrumbs.extend(self.breadcrumbs.iter().cloned());
        event.extra.extend(self.extra.iter().cloned());
        event.tags.extend(self.tags.iter().cloned());
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
            event = processor.process_event(event)?;
        }
        Some(event)
    }
}
