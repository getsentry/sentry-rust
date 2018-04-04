use std::sync::Arc;

use uuid::Uuid;

use api::protocol::Event;
use scope::{with_client_and_scope, with_stack};
use errorlike::current_error_like;

// public api from other crates
pub use sentry_types::{Dsn, ProjectId};
pub use sentry_types::protocol::v7 as protocol;

// public exports from this crate
pub use client::Client;
pub use scope::{pop_scope, push_scope, Scope};
pub use errorlike::ErrorLike;

/// Returns the currently bound client if there is one.
pub fn current_client() -> Option<Arc<Client>> {
    with_stack(|stack| stack.client())
}

/// Rebinds the client on the current scope.
pub fn bind_client(client: Arc<Client>) {
    with_stack(|stack| stack.bind_client(client));
}

/// Captures an event on the currently active client if any.
pub fn capture_event(event: Event) -> Uuid {
    with_client_and_scope(|client, scope| client.capture_event(event, Some(scope)))
}

/// Captures a message with stacktrace.
pub fn capture_exception<E: ErrorLike + ?Sized>(e: Option<&E>) -> Uuid {
    with_client_and_scope(|client, scope| {
        if let Some(e) = e {
            client.capture_exception(e, Some(scope))
        } else {
            client.capture_exception(&*current_error_like(), Some(scope))
        }
    })
}
