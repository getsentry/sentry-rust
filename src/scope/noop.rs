use std::fmt;

use api::protocol::{Context, Event, User, Value};
use hub::Hub;

/// The "shim only" scope.
///
/// In shim only mode all modification functions are available as normally
/// just that generally calling them is impossible.
#[derive(Debug, Clone)]
pub struct Scope;

/// A "shim only" scope guard.
///
/// Doesn't do anything but can be debug formatted.
#[derive(Default)]
pub struct ScopeGuard;

/// A "shim only" scope handle.
///
/// This doesn't do anything.
#[derive(Clone, Default)]
pub struct ScopeHandle;

impl ScopeHandle {
    /// Returns the handle to the current scope.
    pub fn bind(&self) {
        shim_unreachable!();
    }

    /// Binds the scope handle to a specific hub.
    pub fn bind_to_hub<H: AsRef<Hub>>(&self, hub: H) {
        let _hub = hub;
        shim_unreachable!();
    }
}

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

impl Scope {
    /// Clear the scope.
    ///
    /// By default a scope will inherit all values from the higher scope.
    /// In some situations this might not be what a user wants.  Calling
    /// this method will wipe all data contained within.
    pub fn clear(&mut self) {
        shim_unreachable!();
    }

    /// Sets the fingerprint.
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        let _fingerprint = fingerprint;
        shim_unreachable!();
    }

    /// Sets the transaction.
    pub fn set_transaction(&mut self, transaction: Option<&str>) {
        let _transaction = transaction;
        shim_unreachable!();
    }

    /// Sets the user for the current scope.
    pub fn set_user(&mut self, user: Option<User>) {
        let _user = user;
        shim_unreachable!();
    }

    /// Sets a tag to a specific value.
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        let _key = key;
        let _value = value;
        shim_unreachable!();
    }

    /// Removes a tag.
    pub fn remove_tag(&mut self, key: &str) {
        let _key = key;
        shim_unreachable!();
    }

    /// Sets a context for a key.
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        let _key = key;
        let _value = value;
        shim_unreachable!();
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        let _key = key;
        shim_unreachable!();
    }

    /// Sets a extra to a specific value.
    pub fn set_extra(&mut self, key: &str, value: Value) {
        let _key = key;
        let _value = value;
        shim_unreachable!();
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        let _key = key;
        shim_unreachable!();
    }

    /// Applies the contained scoped data to fill an event.
    pub fn apply_to_event(&self, event: &mut Event) {
        let _event = event;
        shim_unreachable!();
    }
}
