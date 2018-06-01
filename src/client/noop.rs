//! The noop fallback client for shim only users.
use uuid::Uuid;

use api::protocol::Event;
use api::Dsn;
use scope::noop::Scope;

/// The "shim only" Sentry client.
///
/// In shim mode the client cannot be constructed and none of the functions can be
/// called.  This is generally irrelevant as there is no way to actually get access
/// to such a client.  However as a result of that not all functions are exposed in
/// that situation.  For instance it's not possible to access the options of the
/// client, but it is possible to access the DSN.
#[derive(Clone)]
pub struct Client;

impl Client {
    /// Returns the DSN that constructed this client.
    ///
    /// Code that works with the shim is permitted to access the DSN to do some operations
    /// on it.
    pub fn dsn(&self) -> &Dsn {
        shim_unreachable!()
    }

    /// Captures an event and sends it to Sentry.
    ///
    /// It is always permissible to send events to sentry from shim user only mode.
    pub fn capture_event(&self, event: Event<'static>, scope: Option<&Scope>) -> Uuid {
        let _event = event;
        let _scope = scope;
        shim_unreachable!()
    }
}
