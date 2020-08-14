use std::borrow::Cow;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use crate::protocol::{Breadcrumb, Context, Event, Level, User, Value};
use crate::session::{Session, SessionUpdate};
use crate::{Client, Envelope};

#[derive(Debug)]
pub struct Stack {
    layers: Vec<StackLayer>,
}

pub type EventProcessor = Box<dyn Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync>;

/// Holds contextual data for the current scope.
///
/// The scope is an object that can be cloned efficiently and stores data that
/// is locally relevant to an event.  For instance the scope will hold recorded
/// breadcrumbs and similar information.
///
/// The scope can be interacted with in two ways:
///
/// 1. the scope is routinely updated with information by functions such as
///    [`add_breadcrumb`] which will modify the currently top-most scope.
/// 2. the topmost scope can also be configured through the [`configure_scope`]
///    method.
///
/// Note that the scope can only be modified but not inspected.  Only the
/// client can use the scope to extract information currently.
///
/// [`add_breadcrumb`]: fn.add_breadcrumb.html
/// [`configure_scope`]: fn.configure_scope.html
#[derive(Clone)]
pub struct Scope {
    pub(crate) level: Option<Level>,
    pub(crate) fingerprint: Option<Arc<Vec<Cow<'static, str>>>>,
    pub(crate) transaction: Option<Arc<String>>,
    pub(crate) breadcrumbs: im::Vector<Breadcrumb>,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) extra: im::HashMap<String, Value>,
    pub(crate) tags: im::HashMap<String, String>,
    pub(crate) contexts: im::HashMap<String, Context>,
    pub(crate) event_processors: im::Vector<Arc<EventProcessor>>,
    pub(crate) session: Option<Arc<Mutex<Session>>>,
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
            .field("session", &self.session)
            .finish()
    }
}

impl Default for Scope {
    fn default() -> Scope {
        Scope {
            level: None,
            fingerprint: None,
            transaction: None,
            breadcrumbs: Default::default(),
            user: None,
            extra: Default::default(),
            tags: Default::default(),
            contexts: Default::default(),
            event_processors: Default::default(),
            session: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StackLayer {
    pub client: Option<Arc<Client>>,
    pub scope: Arc<Scope>,
}

impl Stack {
    pub fn from_client_and_scope(client: Option<Arc<Client>>, scope: Arc<Scope>) -> Stack {
        Stack {
            layers: vec![StackLayer { client, scope }],
        }
    }

    pub fn push(&mut self) {
        let mut layer = self.layers[self.layers.len() - 1].clone();
        // don’t clone the session itself, it should only be on one layer, so
        // that `end`-ing it works correctly.
        let mut scope = Arc::make_mut(&mut layer.scope);
        scope.session = None;
        self.layers.push(layer);
    }

    pub fn pop(&mut self) -> Option<StackLayer> {
        if self.layers.len() <= 1 {
            panic!("Pop from empty stack");
        }
        self.layers.pop()
    }

    pub fn top(&self) -> &StackLayer {
        &self.layers[self.layers.len() - 1]
    }

    pub fn top_mut(&mut self) -> &mut StackLayer {
        let top = self.layers.len() - 1;
        &mut self.layers[top]
    }

    pub fn depth(&self) -> usize {
        self.layers.len()
    }
}

/// A scope guard.
///
/// This is returned from [`Hub::push_scope`] and will automatically pop the
/// scope on drop.
///
/// [`Hub::push_scope`]: struct.Hub.html#method.with_scope
#[derive(Default)]
pub struct ScopeGuard(pub(crate) Option<(Arc<RwLock<Stack>>, usize)>);

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        if let Some((stack, depth)) = self.0.take() {
            let mut stack = stack.write().unwrap_or_else(PoisonError::into_inner);
            if stack.depth() != depth {
                panic!("Tried to pop guards out of order");
            }
            let mut layer = stack.pop().unwrap();
            (|| {
                let scope = Arc::make_mut(&mut layer.scope);
                let mut session = Arc::try_unwrap(scope.session.take()?)
                    .ok()?
                    .into_inner()
                    .ok()?;
                let client = layer.client.as_ref()?;

                session.close();
                let mut envelope = Envelope::new();
                envelope.add(session.into());
                client.capture_envelope(envelope);
                None::<()>
            })();
        }
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
    pub fn add_event_processor(
        &mut self,
        f: Box<dyn Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync>,
    ) {
        self.event_processors.push_back(Arc::new(f));
    }

    /// Applies the contained scoped data to fill an event.
    #[allow(clippy::cognitive_complexity)]
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
            let id = event.event_id;
            event = match processor(event) {
                Some(event) => event,
                None => {
                    sentry_debug!("event processor dropped event {}", id);
                    return None;
                }
            }
        }

        Some(event)
    }

    pub(crate) fn update_session_from_event(&self, event: &Event<'static>) -> SessionUpdate {
        if let Some(session) = &self.session {
            session.lock().unwrap().update_from_event(event)
        } else {
            SessionUpdate::Unchanged
        }
    }
}
