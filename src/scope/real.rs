use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::mem;
use std::sync::{Arc, RwLock};
use std::thread;

use api::protocol::{Breadcrumb, Context, User, Value};
use client::Client;

use im;

lazy_static! {
    static ref PROCESS_STACK: RwLock<Stack> = RwLock::new(Stack::for_process());
}
thread_local! {
    static THREAD_STACK: RefCell<Stack> = RefCell::new(Stack::for_thread());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StackType {
    Process,
    Thread,
}

#[derive(Debug)]
pub struct Stack {
    layers: Vec<StackLayer>,
    ty: StackType,
}

#[derive(PartialEq, Clone, Copy)]
struct StackLayerToken(*const Stack, usize);

#[allow(unused)]
pub(crate) fn scope_panic_safe() -> bool {
    PROCESS_STACK.try_read().is_ok() && THREAD_STACK.with(|x| x.try_borrow().is_ok())
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
#[derive(Debug, Clone)]
pub struct Scope {
    pub(crate) fingerprint: Option<Arc<Vec<Cow<'static, str>>>>,
    pub(crate) transaction: Option<Arc<String>>,
    pub(crate) breadcrumbs: im::Vector<Breadcrumb>,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) extra: im::HashMap<String, Value>,
    pub(crate) tags: im::HashMap<String, String>,
    pub(crate) contexts: im::HashMap<String, Option<Context>>,
}

fn default_scope() -> Scope {
    Scope {
        fingerprint: None,
        transaction: None,
        breadcrumbs: Default::default(),
        user: None,
        extra: Default::default(),
        tags: Default::default(),
        contexts: Default::default(),
    }
}

#[derive(Debug, Clone)]
struct StackLayer {
    client: Option<Arc<Client>>,
    scope: Scope,
}

impl Stack {
    pub fn for_process() -> Stack {
        Stack {
            layers: vec![StackLayer {
                client: None,
                scope: default_scope(),
            }],
            ty: StackType::Process,
        }
    }

    pub fn for_thread() -> Stack {
        Stack {
            layers: vec![with_process_stack(|stack| StackLayer {
                client: stack.client(),
                scope: stack.scope().clone(),
            })],
            ty: StackType::Thread,
        }
    }

    pub fn push(&mut self) {
        let scope = self.layers[self.layers.len() - 1].clone();
        self.layers.push(scope);
    }

    pub fn pop(&mut self) {
        if self.layers.len() <= 1 {
            panic!("Pop from empty {:?} stack", self.ty)
        }
        self.layers.pop().unwrap();
    }

    pub fn bind_client(&mut self, client: Option<Arc<Client>>) {
        let depth = self.layers.len() - 1;
        self.layers[depth].client = client;
    }

    pub fn client(&self) -> Option<Arc<Client>> {
        self.layers[self.layers.len() - 1].client.clone()
    }

    pub fn scope(&self) -> &Scope {
        let idx = self.layers.len() - 1;
        &self.layers[idx].scope
    }

    pub fn scope_mut(&mut self) -> &mut Scope {
        let idx = self.layers.len() - 1;
        &mut self.layers[idx].scope
    }

    fn layer_token(&self) -> StackLayerToken {
        StackLayerToken(self as *const Stack, self.layers.len())
    }
}

fn is_main_thread() -> bool {
    let thread = thread::current();
    let raw_id: u64 = unsafe { mem::transmute(thread.id()) };
    raw_id == 0
}

fn with_process_stack<F, R>(f: F) -> R
where
    F: FnOnce(&Stack) -> R,
{
    f(&mut PROCESS_STACK.read().unwrap_or_else(|x| x.into_inner()))
}

fn with_process_stack_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Stack) -> R,
{
    f(&mut PROCESS_STACK.write().unwrap_or_else(|x| x.into_inner()))
}

fn with_stack_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Stack) -> R,
{
    if is_main_thread() {
        with_process_stack_mut(f)
    } else {
        THREAD_STACK.with(|stack| f(&mut *stack.borrow_mut()))
    }
}

fn with_stack<F, R>(f: F) -> R
where
    F: FnOnce(&Stack) -> R,
{
    if is_main_thread() {
        with_process_stack(f)
    } else {
        THREAD_STACK.with(|stack| f(&*stack.borrow()))
    }
}

/// Invokes a function if the sentry client is available with client and scope.
///
/// The function is invoked with the client and current scope (read-only) and permits
/// operations to be executed on the client.  This is useful when writing integration
/// code where potentially expensive operations should not be executed if Sentry is
/// not configured.
///
/// The return value must be `Default` so that it can be created even if Sentry is not
/// configured.
pub fn with_client_and_scope<F, R>(f: F) -> R
where
    F: FnOnce(Arc<Client>, &Scope) -> R,
    R: Default,
{
    with_stack(|stack| {
        if let Some(client) = stack.client() {
            f(client, stack.scope())
        } else {
            Default::default()
        }
    })
}

/// Crate internal helper for working with clients and mutable scopes.
pub(crate) fn with_client_and_scope_mut<F, R>(f: F) -> R
where
    F: FnOnce(Arc<Client>, &mut Scope) -> R,
    R: Default,
{
    with_stack_mut(|stack| {
        if let Some(client) = stack.client() {
            f(client, stack.scope_mut())
        } else {
            Default::default()
        }
    })
}

/// A scope guard.
#[derive(Default)]
pub struct ScopeGuard(Option<StackLayerToken>);

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        if let Some(token) = self.0 {
            with_stack_mut(|stack| {
                if stack.layer_token() != token {
                    panic!("Current active stack does not match scope guard");
                } else {
                    stack.pop();
                }
            })
        }
    }
}

/// Pushes a new scope on the stack.
///
/// The currently bound client is propagated to the new scope and already existing
/// data on the scope is inherited.  Modifications done to the inner scope however
/// are isolated from the outer scope.
///
/// This returns a guard.  When the guard is collected the scope is popped again.
///
/// # Example
///
/// ```no_run
/// {
///     let _guard = sentry::push_scope();
///     sentry::configure_scope(|scope| {
///         scope.set_tag("some_tag", "some_value");
///     });
///     // until the end of the block the scope is changed.
/// }
/// ```
pub fn push_scope() -> ScopeGuard {
    with_stack_mut(|stack| {
        stack.push();
        ScopeGuard(Some(stack.layer_token()))
    })
}

/// Returns the currently bound client if there is one.
///
/// This might return `None` in case there is no client.  For the most part
/// code will not use this function but instead directly call `capture_event`
/// and similar functions which work on the currently active client.
pub fn current_client() -> Option<Arc<Client>> {
    with_client_impl! {{
        with_stack(|stack| stack.client())
    }}
}

/// Rebinds the client on the current scope.
///
/// The current scope is defined as the current thread.  If a new thread spawns
/// it inherits the client of the process.  The main thread is specially handled
/// in the sense that if the main thread binds a client it becomes bound to the
/// process.
pub fn bind_client(client: Arc<Client>) {
    with_client_impl! {{
        with_stack_mut(|stack| stack.bind_client(Some(client)));
    }}
}

/// Unbinds the client on the current scope.
///
/// This effectively prevents data collection and reporting on the current scope.
pub fn unbind_client() {
    with_client_impl! {{
        with_stack_mut(|stack| stack.bind_client(None));
    }}
}

impl Scope {
    /// Clear the scope.
    ///
    /// By default a scope will inherit all values from the higher scope.
    /// In some situations this might not be what a user wants.  Calling
    /// this method will wipe all data contained within.
    pub fn clear(&mut self) {
        *self = default_scope();
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
}
