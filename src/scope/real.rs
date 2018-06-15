use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use api::protocol::map::Entry;
use api::protocol::{Breadcrumb, Context, Event, User, Value};
use client::Client;
use hub::Hub;
use utils;

use im;

lazy_static! {
    static ref CONTEXT_DEFAULTS: ContextDefaults = ContextDefaults {
        os: utils::os_context(),
        rust: utils::rust_context(),
        device: utils::device_context(),
    };
}

#[derive(Debug)]
struct ContextDefaults {
    pub os: Option<Context>,
    pub rust: Option<Context>,
    pub device: Option<Context>,
}

#[derive(Debug)]
pub struct Stack {
    layers: Vec<StackLayer>,
}

#[derive(PartialEq, Clone, Copy)]
pub struct StackLayerToken(*const Stack, usize);

/// Returns the handle to the current scope.
///
/// This can be used to propagate a scope to another thread easily. The parent
/// thread retrieves a handle and the child thread binds it. A handle can be
/// cloned so that it can be used in multiple threads.
#[derive(Clone, Default)]
pub struct ScopeHandle(pub(crate) Option<Arc<Scope>>);

impl ScopeHandle {
    /// Returns the handle to the current scope.
    pub fn bind(self) {
        Hub::with(|hub| self.bind_to_hub(hub))
    }

    /// Binds the scope handle to a specific hub.
    pub fn bind_to_hub<H: AsRef<Hub>>(self, hub: H) {
        if let Some(other_scope) = self.0 {
            hub.as_ref()
                .with_scope_mut(|scope| *scope = (*other_scope).clone());
        }
    }
}

/// Holds contextual data for the current scope.
///
/// The scope is an object that can cloned efficiently and stores data that
/// is locally relevant to an event.  For instance the scope will hold recorded
/// breadcrumbs and similar information.
///
/// The scope can be interacted with in two ways:
///
/// 1. the scope is routinely updated with information by functions such as
///    `add_breadcrumb` which will modify the currently top-most scope.
/// 2. the topmost scope can also be configured through the `configure_scope`
///    method.
///
/// Note that the scope can only be modified but not inspected.  Only the
/// client can use the scope to extract information currently.
#[derive(Clone)]
pub struct Scope {
    pub(crate) fingerprint: Option<Arc<Vec<Cow<'static, str>>>>,
    pub(crate) transaction: Option<Arc<String>>,
    pub(crate) breadcrumbs: im::Vector<Breadcrumb>,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) extra: im::HashMap<String, Value>,
    pub(crate) tags: im::HashMap<String, String>,
    pub(crate) contexts: im::HashMap<String, Option<Context>>,
    pub(crate) event_processors: im::Vector<Box<Fn(&mut Event) + Send + Sync>>,
}

impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Scope")
            .field("fingerprint", &self.fingerprint)
            .field("transaction", &self.transaction)
            .field("breadcrumbs", &self.breadcrumbs)
            .field("user", &self.user)
            .field("extra", &self.extra)
            .field("tags", &self.tags)
            .field("contexts", &self.contexts)
            .finish()
    }
}

impl Default for Scope {
    fn default() -> Scope {
        Scope {
            fingerprint: None,
            transaction: None,
            breadcrumbs: Default::default(),
            user: None,
            extra: Default::default(),
            tags: Default::default(),
            contexts: Default::default(),
            event_processors: Default::default(),
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
        let scope = self.layers[self.layers.len() - 1].clone();
        self.layers.push(scope);
    }

    pub fn pop(&mut self) {
        if self.layers.len() <= 1 {
            panic!("Pop from empty stack");
        }
        self.layers.pop().unwrap();
    }

    pub fn top(&self) -> &StackLayer {
        &self.layers[self.layers.len() - 1]
    }

    pub fn top_mut(&mut self) -> &mut StackLayer {
        let top = self.layers.len() - 1;
        &mut self.layers[top]
    }

    pub fn layer_token(&self) -> StackLayerToken {
        StackLayerToken(self as *const Stack, self.layers.len())
    }
}

/// A scope guard.
#[derive(Default)]
pub struct ScopeGuard(pub(crate) Option<StackLayerToken>);

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        if let Some(token) = self.0 {
            Hub::with(|hub| hub.pop_scope(token))
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

    /// Sets the fingerprint.
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        self.fingerprint =
            fingerprint.map(|fp| Arc::new(fp.iter().map(|x| Cow::Owned(x.to_string())).collect()))
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
    #[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        self.tags = self.tags.insert(key.to_string(), value.to_string());
    }

    /// Removes a tag.
    pub fn remove_tag(&mut self, key: &str) {
        // annoyingly this needs a String :(
        self.tags = self.tags.remove(&key.to_string());
    }

    /// Sets a context for a key.
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        self.contexts = self.contexts.insert(key.to_string(), Some(value.into()));
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        // annoyingly this needs a String :(
        self.contexts = self.contexts.insert(&key.to_string(), None);
    }

    /// Sets a extra to a specific value.
    pub fn set_extra(&mut self, key: &str, value: Value) {
        self.extra = self.extra.insert(key.to_string(), value);
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        // annoyingly this needs a String :(
        self.extra = self.extra.remove(&key.to_string());
    }

    /// Applies the contained scoped data to fill an event.
    pub fn apply_to_event(&self, event: &mut Event) {
        let mut add_os = true;
        let mut add_rust = true;
        let mut add_device = true;

        if !self.breadcrumbs.is_empty() {
            event
                .breadcrumbs
                .extend(self.breadcrumbs.iter().map(|x| (*x).clone()));
        }

        if event.user.is_none() {
            if let Some(ref user) = self.user {
                event.user = Some((**user).clone());
            }
        }

        if !self.extra.is_empty() {
            event
                .extra
                .extend(self.extra.iter().map(|(k, v)| ((*k).clone(), (*v).clone())));
        }

        if !self.tags.is_empty() {
            event
                .tags
                .extend(self.tags.iter().map(|(k, v)| ((*k).clone(), (*v).clone())));
        }

        for (key, value) in self.contexts.iter() {
            match *value {
                None => match key.as_str() {
                    "os" => add_os = false,
                    "rust" => add_rust = false,
                    "device" => add_device = false,
                    _ => {}
                },
                Some(ref value) => {
                    event
                        .contexts
                        .entry((*key).clone())
                        .or_insert_with(|| (*value).clone());
                }
            }
        }

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

        if add_os {
            if let Entry::Vacant(entry) = event.contexts.entry("os".to_string()) {
                if let Some(ref os) = CONTEXT_DEFAULTS.os {
                    entry.insert(os.clone());
                }
            }
        }

        if add_rust {
            if let Entry::Vacant(entry) = event.contexts.entry("rust".to_string()) {
                if let Some(ref rust) = CONTEXT_DEFAULTS.rust {
                    entry.insert(rust.clone());
                }
            }
        }

        if add_device {
            if let Entry::Vacant(entry) = event.contexts.entry("device".to_string()) {
                if let Some(ref device) = CONTEXT_DEFAULTS.device {
                    entry.insert(device.clone());
                }
            }
        }
    }
}
