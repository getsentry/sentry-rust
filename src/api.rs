use std::sync::Arc;

use uuid::Uuid;

use api::protocol::{Breadcrumb, Event};
use scope::{with_client_and_scope, with_stack};

// public api from other crates
pub use sentry_types::{Dsn, ProjectId};
pub use sentry_types::protocol::v7 as protocol;

// public exports from this crate
pub use client::{Client, ClientOptions};
pub use scope::{pop_scope, push_scope};

/// Helper trait to convert an object into an DSN.
pub trait IntoDsn {
    /// Convert the value into a DSN.
    ///
    /// In case the null DSN shold be used `None` must be returned.
    fn into_dsn(self) -> Option<Dsn>;
}

impl<'a> IntoDsn for &'a str {
    fn into_dsn(self) -> Option<Dsn> {
        Some(self.parse().unwrap())
    }
}

impl<'a> IntoDsn for Dsn {
    fn into_dsn(self) -> Option<Dsn> {
        Some(self)
    }
}

/// Creates the sentry client and binds it.
pub fn create<I: IntoDsn>(dsn: Option<I>, options: Option<ClientOptions>) {
    let client = if let Some(dsn) = dsn.and_then(|x| x.into_dsn()) {
        if let Some(options) = options {
            Some(Client::with_options(dsn, options))
        } else {
            Some(Client::new(dsn))
        }
    } else {
        None
    };
    if let Some(client) = client {
        bind_client(Arc::new(client));
    }
}

/// Returns the currently bound client if there is one.
///
/// This might return `None` in case there is no client.  For the most part
/// code will not use this function but instead directly call `capture_event`
/// and similar functions which work on the currently active client.
pub fn current_client() -> Option<Arc<Client>> {
    with_stack(|stack| stack.client())
}

/// Rebinds the client on the current scope.
///
/// The current scope is defined as the current thread.  If a new thread spawns
/// it inherits the client of the process.  The main thread is specially handled
/// in the sense that if the main thread binds a client it becomes bound to the
/// process.
pub fn bind_client(client: Arc<Client>) {
    with_stack(|stack| stack.bind_client(client));
}

/// Captures an event on the currently active client if any.
///
/// The event must already be assembled.  Typically code would instead use
/// the utility methods like `capture_exception`.
pub fn capture_event(event: Event) -> Uuid {
    with_client_and_scope(|client, scope| client.capture_event(event, Some(scope)))
}

/// Records a breadcrumb.
///
/// The total number of breadcrumbs that can be recorded are limited by the
/// configuration on the client.
pub fn add_breadcrumb(bc: Breadcrumb) {
    with_client_and_scope(|client, scope| {
        scope.breadcrumbs.push_back(bc);
        let limit = client.options().max_breadcrumbs;
        while scope.breadcrumbs.len() > limit {
            scope.breadcrumbs.pop_front();
        }
    })
}

/// Records a breadcrumb by calling a function.
///
/// The total number of breadcrumbs that can be recorded are limited by the
/// configuration on the client.
pub fn add_breadcrumb_from<F: FnOnce() -> Breadcrumb>(f: F) {
    with_client_and_scope(|client, scope| {
        scope.breadcrumbs.push_back(f());
        let limit = client.options().max_breadcrumbs;
        while scope.breadcrumbs.len() > limit {
            scope.breadcrumbs.pop_front();
        }
    })
}
