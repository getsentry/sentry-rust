use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fmt;
#[cfg(feature = "release-health")]
use std::sync::Mutex;
use std::sync::{Arc, PoisonError, RwLock};

use crate::performance::TransactionOrSpan;
use crate::protocol::{
    Attachment, Breadcrumb, Context, Event, Level, TraceContext, Transaction, User, Value,
};
#[cfg(feature = "logs")]
use crate::protocol::{Log, LogAttribute};
#[cfg(feature = "release-health")]
use crate::session::Session;
use crate::{Client, SentryTrace, TraceHeader, TraceHeadersIter};
use crate::sentry_debug;

#[derive(Debug)]
pub struct Stack {
    top: StackLayer,
    layers: Vec<StackLayer>,
}

pub type EventProcessor = Arc<dyn Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync>;

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
#[derive(Clone, Default)]
pub struct Scope {
    pub(crate) level: Option<Level>,
    pub(crate) fingerprint: Option<Arc<[Cow<'static, str>]>>,
    pub(crate) transaction: Option<Arc<str>>,
    pub(crate) breadcrumbs: Arc<VecDeque<Breadcrumb>>,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) extra: Arc<HashMap<String, Value>>,
    pub(crate) tags: Arc<HashMap<String, String>>,
    pub(crate) contexts: Arc<HashMap<String, Context>>,
    pub(crate) event_processors: Arc<Vec<EventProcessor>>,
    #[cfg(feature = "release-health")]
    pub(crate) session: Arc<Mutex<Option<Session>>>,
    pub(crate) span: Arc<Option<TransactionOrSpan>>,
    pub(crate) attachments: Arc<Vec<Attachment>>,
    pub(crate) propagation_context: SentryTrace,
}

impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_struct = f.debug_struct("Scope");
        debug_struct
            .field("level", &self.level)
            .field("fingerprint", &self.fingerprint)
            .field("transaction", &self.transaction)
            .field("breadcrumbs", &self.breadcrumbs)
            .field("user", &self.user)
            .field("extra", &self.extra)
            .field("tags", &self.tags)
            .field("contexts", &self.contexts)
            .field("event_processors", &self.event_processors.len());

        #[cfg(feature = "release-health")]
        debug_struct.field("session", &self.session);

        debug_struct
            .field("span", &self.span)
            .field("attachments", &self.attachments.len())
            .field("propagation_context", &self.propagation_context)
            .finish()
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
            top: StackLayer { client, scope },
            layers: vec![],
        }
    }

    pub fn push(&mut self) {
        let layer = self.top.clone();
        self.layers.push(layer);
    }

    pub fn pop(&mut self) {
        if self.layers.is_empty() {
            panic!("Pop from empty stack");
        }
        self.top = self.layers.pop().unwrap();
    }

    #[inline(always)]
    pub fn top(&self) -> &StackLayer {
        &self.top
    }

    #[inline(always)]
    pub fn top_mut(&mut self) -> &mut StackLayer {
        &mut self.top
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
            let popped_depth = {
                let mut stack = stack.write().unwrap_or_else(PoisonError::into_inner);
                let popped_depth = stack.depth();
                stack.pop();
                popped_depth
            };
            // NOTE: We need to drop the `stack` lock before panicking, as the
            // `PanicIntegration` will want to lock the `stack` itself
            // (through `capture_event` -> `HubImpl::with`), and would thus
            // result in a deadlock.
            // Though that deadlock itself is detected by the `RwLock` (on macOS)
            // and results in its own panic: `rwlock read lock would result in deadlock`.
            // However that panic happens in the panic handler and will thus
            // ultimately result in a `thread panicked while processing panic. aborting.`
            // Long story short, we should not panic while holding the lock :-)
            if popped_depth != depth {
                panic!("Popped scope guard out of order");
            }
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
        sentry_debug!("[Scope] Clearing all scope data");
        *self = Default::default();
    }

    /// Deletes current breadcrumbs from the scope.
    pub fn clear_breadcrumbs(&mut self) {
        let previous_count = self.breadcrumbs.len();
        self.breadcrumbs = Default::default();
        sentry_debug!("[Scope] Cleared {} breadcrumbs", previous_count);
    }

    /// Sets a level override.
    pub fn set_level(&mut self, level: Option<Level>) {
        match (&self.level, &level) {
            (None, Some(new_level)) => sentry_debug!("[Scope] Setting level override: {:?}", new_level),
            (Some(old_level), Some(new_level)) if old_level != new_level => 
                sentry_debug!("[Scope] Changing level override: {:?} -> {:?}", old_level, new_level),
            (Some(_), None) => sentry_debug!("[Scope] Removing level override"),
            _ => {}
        }
        self.level = level;
    }

    /// Sets the fingerprint.
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        if let Some(fp) = fingerprint {
            sentry_debug!("[Scope] Setting fingerprint: {:?}", fp);
        } else {
            sentry_debug!("[Scope] Removing fingerprint");
        }
        self.fingerprint =
            fingerprint.map(|fp| fp.iter().map(|s| Cow::Owned((*s).into())).collect())
    }

    /// Sets the transaction.
    pub fn set_transaction(&mut self, transaction: Option<&str>) {
        match (&self.transaction, transaction) {
            (None, Some(new_tx)) => sentry_debug!("[Scope] Setting transaction: {}", new_tx),
            (Some(old_tx), Some(new_tx)) if old_tx.as_ref() != new_tx => 
                sentry_debug!("[Scope] Changing transaction: {} -> {}", old_tx, new_tx),
            (Some(_), None) => sentry_debug!("[Scope] Removing transaction"),
            _ => {}
        }
        
        self.transaction = transaction.map(Arc::from);
        if let Some(name) = transaction {
            let trx = match self.span.as_ref() {
                Some(TransactionOrSpan::Span(span)) => &span.transaction,
                Some(TransactionOrSpan::Transaction(trx)) => &trx.inner,
                _ => return,
            };

            if let Some(trx) = trx.lock().unwrap().transaction.as_mut() {
                trx.name = Some(name.into());
                sentry_debug!("[Scope] Updated active transaction name: {}", name);
            }
        }
    }

    /// Sets the user for the current scope.
    pub fn set_user(&mut self, user: Option<User>) {
        match (&self.user, &user) {
            (None, Some(new_user)) => sentry_debug!("[Scope] Setting user: id={:?}, username={:?}, email={:?}", 
                                                   new_user.id, new_user.username, new_user.email),
            (Some(_), Some(new_user)) => sentry_debug!("[Scope] Updating user: id={:?}, username={:?}, email={:?}", 
                                                       new_user.id, new_user.username, new_user.email),
            (Some(_), None) => sentry_debug!("[Scope] Removing user"),
            _ => {}
        }
        self.user = user.map(Arc::new);
    }

    /// Retrieves the user of the current scope.
    pub fn user(&self) -> Option<&User> {
        self.user.as_deref()
    }

    /// Sets a tag to a specific value.
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        let value_str = value.to_string();
        sentry_debug!("[Scope] Setting tag: {} = {}", key, value_str);
        Arc::make_mut(&mut self.tags).insert(key.to_string(), value_str);
    }

    /// Removes a tag.
    ///
    /// If the tag is not set, does nothing.
    pub fn remove_tag(&mut self, key: &str) {
        if Arc::make_mut(&mut self.tags).remove(key).is_some() {
            sentry_debug!("[Scope] Removed tag: {}", key);
        }
    }

    /// Sets a context for a key.
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        sentry_debug!("[Scope] Setting context: {}", key);
        Arc::make_mut(&mut self.contexts).insert(key.to_string(), value.into());
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        if Arc::make_mut(&mut self.contexts).remove(key).is_some() {
            sentry_debug!("[Scope] Removed context: {}", key);
        }
    }

    /// Sets a extra to a specific value.
    pub fn set_extra(&mut self, key: &str, value: Value) {
        sentry_debug!("[Scope] Setting extra: {} = {:?}", key, value);
        Arc::make_mut(&mut self.extra).insert(key.to_string(), value);
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        if Arc::make_mut(&mut self.extra).remove(key).is_some() {
            sentry_debug!("[Scope] Removed extra: {}", key);
        }
    }

    /// Add an event processor to the scope.
    pub fn add_event_processor<F>(&mut self, f: F)
    where
        F: Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        Arc::make_mut(&mut self.event_processors).push(Arc::new(f));
        sentry_debug!("[Scope] Added event processor (total: {})", self.event_processors.len());
    }

    /// Adds an attachment to the scope
    pub fn add_attachment(&mut self, attachment: Attachment) {
        let filename = attachment.filename.clone().unwrap_or_else(|| "<unnamed>".to_string());
        Arc::make_mut(&mut self.attachments).push(attachment);
        sentry_debug!("[Scope] Added attachment: {} (total: {})", filename, self.attachments.len());
    }

    /// Clears attachments from the scope
    pub fn clear_attachments(&mut self) {
        let previous_count = self.attachments.len();
        Arc::make_mut(&mut self.attachments).clear();
        sentry_debug!("[Scope] Cleared {} attachments", previous_count);
    }

    /// Applies the contained scoped data to fill an event.
    pub fn apply_to_event(&self, mut event: Event<'static>) -> Option<Event<'static>> {
        sentry_debug!("[Scope] Applying scope to event {}", event.event_id);
        
        // TODO: event really should have an optional level
        if self.level.is_some() {
            event.level = self.level.unwrap();
            sentry_debug!("[Scope] Applied level override: {:?}", self.level.unwrap());
        }

        if event.user.is_none() {
            if let Some(user) = self.user.as_deref() {
                event.user = Some(user.clone());
                sentry_debug!("[Scope] Applied user to event");
            }
        }

        let breadcrumb_count = self.breadcrumbs.len();
        if breadcrumb_count > 0 {
            event.breadcrumbs.extend(self.breadcrumbs.iter().cloned());
            sentry_debug!("[Scope] Applied {} breadcrumbs to event", breadcrumb_count);
        }
        
        let extra_count = self.extra.len();
        if extra_count > 0 {
            event
                .extra
                .extend(self.extra.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
            sentry_debug!("[Scope] Applied {} extra fields to event", extra_count);
        }
        
        let tag_count = self.tags.len();
        if tag_count > 0 {
            event
                .tags
                .extend(self.tags.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
            sentry_debug!("[Scope] Applied {} tags to event", tag_count);
        }
        
        let context_count = self.contexts.len();
        if context_count > 0 {
            event.contexts.extend(
                self.contexts
                    .iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned())),
            );
            sentry_debug!("[Scope] Applied {} contexts to event", context_count);
        }

        if let Some(span) = self.span.as_ref() {
            span.apply_to_event(&mut event);
            sentry_debug!("[Scope] Applied span context to event");
        } else {
            self.apply_propagation_context(&mut event);
            sentry_debug!("[Scope] Applied propagation context to event");
        }

        if event.transaction.is_none() {
            if let Some(txn) = self.transaction.as_deref() {
                event.transaction = Some(txn.to_owned());
                sentry_debug!("[Scope] Applied transaction name to event: {}", txn);
            }
        }

        if event.fingerprint.len() == 1
            && (event.fingerprint[0] == "{{ default }}" || event.fingerprint[0] == "{{default}}")
        {
            if let Some(fp) = self.fingerprint.as_deref() {
                event.fingerprint = Cow::Owned(fp.to_owned());
                sentry_debug!("[Scope] Applied custom fingerprint to event");
            }
        }

        sentry_debug!("[Scope] Processing event through {} event processors", self.event_processors.len());
        for (i, processor) in self.event_processors.as_ref().iter().enumerate() {
            let id = event.event_id;
            event = match processor(event) {
                Some(event) => event,
                None => {
                    sentry_debug!("[Scope] Event processor {} dropped event {}", i + 1, id);
                    return None;
                }
            }
        }

        sentry_debug!("[Scope] Successfully applied scope to event {}", event.event_id);
        Some(event)
    }

    /// Applies the contained scoped data to fill a transaction.
    pub fn apply_to_transaction(&self, transaction: &mut Transaction<'static>) {
        sentry_debug!("[Scope] Applying scope to transaction");
        
        if transaction.user.is_none() {
            if let Some(user) = self.user.as_deref() {
                transaction.user = Some(user.clone());
                sentry_debug!("[Scope] Applied user to transaction");
            }
        }

        let extra_count = self.extra.len();
        if extra_count > 0 {
            transaction
                .extra
                .extend(self.extra.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
            sentry_debug!("[Scope] Applied {} extra fields to transaction", extra_count);
        }
        
        let tag_count = self.tags.len();
        if tag_count > 0 {
            transaction
                .tags
                .extend(self.tags.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
            sentry_debug!("[Scope] Applied {} tags to transaction", tag_count);
        }
        
        let context_count = self.contexts.len();
        if context_count > 0 {
            transaction.contexts.extend(
                self.contexts
                    .iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned())),
            );
            sentry_debug!("[Scope] Applied {} contexts to transaction", context_count);
        }
    }

    /// Applies the contained scoped data to a log, setting the `trace_id` and certain default
    /// attributes.
    #[cfg(feature = "logs")]
    pub fn apply_to_log(&self, log: &mut Log, send_default_pii: bool) {
        sentry_debug!("[Scope] Applying scope to log (send_default_pii: {})", send_default_pii);
        
        if let Some(span) = self.span.as_ref() {
            log.trace_id = Some(span.get_trace_context().trace_id);
            sentry_debug!("[Scope] Applied trace_id from span to log");
        } else {
            log.trace_id = Some(self.propagation_context.trace_id);
            sentry_debug!("[Scope] Applied trace_id from propagation context to log");
        }

        if !log.attributes.contains_key("sentry.trace.parent_span_id") {
            if let Some(span) = self.get_span() {
                let span_id = match span {
                    crate::TransactionOrSpan::Transaction(transaction) => {
                        transaction.get_trace_context().span_id
                    }
                    crate::TransactionOrSpan::Span(span) => span.get_span_id(),
                };
                log.attributes.insert(
                    "parent_span_id".to_owned(),
                    LogAttribute(span_id.to_string().into()),
                );
                sentry_debug!("[Scope] Applied parent_span_id to log");
            }
        }

        if send_default_pii {
            if let Some(user) = self.user.as_ref() {
                let mut added_user_attrs = Vec::new();
                
                if !log.attributes.contains_key("user.id") {
                    if let Some(id) = user.id.as_ref() {
                        log.attributes
                            .insert("user.id".to_owned(), LogAttribute(id.to_owned().into()));
                        added_user_attrs.push("user.id");
                    }
                }

                if !log.attributes.contains_key("user.name") {
                    if let Some(name) = user.username.as_ref() {
                        log.attributes
                            .insert("user.name".to_owned(), LogAttribute(name.to_owned().into()));
                        added_user_attrs.push("user.name");
                    }
                }

                if !log.attributes.contains_key("user.email") {
                    if let Some(email) = user.email.as_ref() {
                        log.attributes.insert(
                            "user.email".to_owned(),
                            LogAttribute(email.to_owned().into()),
                        );
                        added_user_attrs.push("user.email");
                    }
                }
                
                if !added_user_attrs.is_empty() {
                    sentry_debug!("[Scope] Applied user attributes to log: {}", added_user_attrs.join(", "));
                }
            }
        }
    }

    /// Set the given [`TransactionOrSpan`] as the active span for this scope.
    pub fn set_span(&mut self, span: Option<TransactionOrSpan>) {
        match (&self.span.as_ref(), &span) {
            (None, Some(_)) => sentry_debug!("[Scope] Setting active span"),
            (Some(_), Some(_)) => sentry_debug!("[Scope] Replacing active span"),
            (Some(_), None) => sentry_debug!("[Scope] Removing active span"),
            _ => {}
        }
        self.span = Arc::new(span);
    }

    /// Returns the currently active span.
    pub fn get_span(&self) -> Option<TransactionOrSpan> {
        self.span.as_ref().clone()
    }

    #[allow(unused_variables)]
    pub(crate) fn update_session_from_event(&self, event: &Event<'static>) {
        #[cfg(feature = "release-health")]
        if let Some(session) = self.session.lock().unwrap().as_mut() {
            sentry_debug!("[Scope] Updating session from event {}", event.event_id);
            session.update_from_event(event);
        }
    }

    pub(crate) fn apply_propagation_context(&self, event: &mut Event<'_>) {
        if event.contexts.contains_key("trace") {
            return;
        }

        let context = TraceContext {
            trace_id: self.propagation_context.trace_id,
            span_id: self.propagation_context.span_id,
            ..Default::default()
        };
        event.contexts.insert("trace".into(), context.into());
    }

    /// Returns the headers needed for distributed tracing.
    pub fn iter_trace_propagation_headers(&self) -> impl Iterator<Item = TraceHeader> {
        if let Some(span) = self.get_span() {
            span.iter_headers()
        } else {
            let data = SentryTrace::new(
                self.propagation_context.trace_id,
                self.propagation_context.span_id,
                None,
            );
            TraceHeadersIter::new(data.to_string())
        }
    }
}
