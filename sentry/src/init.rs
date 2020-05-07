use std::sync::Arc;

use sentry_core::sentry_debug;

use crate::{defaults::apply_defaults, Client, ClientOptions, Hub};

/// Helper struct that is returned from `init`.
///
/// When this is dropped events are drained with a 1 second timeout.
#[must_use = "when the init guard is dropped the transport will be shut down and no further \
              events can be sent.  If you do want to ignore this use mem::forget on it."]
pub struct ClientInitGuard(Arc<Client>);

impl std::ops::Deref for ClientInitGuard {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClientInitGuard {
    /// Quick check if the client is enabled.
    pub fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }
}

impl Drop for ClientInitGuard {
    fn drop(&mut self) {
        if self.is_enabled() {
            sentry_debug!("dropping client guard -> disposing client");
        } else {
            sentry_debug!("dropping client guard (no client to dispose)");
        }
        self.0.close(None);
    }
}

/// Creates the Sentry client for a given client config and binds it.
///
/// This returns a client init guard that must kept in scope will help the
/// client send events before the application closes.  When the guard is
/// dropped then the transport that was initialized shuts down and no
/// further events can be sent on it.
///
/// If you don't want (or can) keep the guard around it's permissible to
/// call `mem::forget` on it.
///
/// # Examples
///
/// ```
/// let _sentry = sentry::init("https://key@sentry.io/1234");
/// ```
///
/// Or if draining on shutdown should be ignored:
///
/// ```
/// std::mem::forget(sentry::init("https://key@sentry.io/1234"));
/// ```
///
/// The guard returned can also be inspected to see if a client has been
/// created to enable further configuration:
///
/// ```
/// let sentry = sentry::init(sentry::ClientOptions {
///     release: Some("foo-bar-baz@1.0.0".into()),
///     ..Default::default()
/// });
/// if sentry.is_enabled() {
///     // some other initialization
/// }
/// ```
///
/// This behaves similar to creating a client by calling `Client::from_config`
/// and to then bind it to the hub except it also applies default integrations,
/// a default transport, as well as other options populated from environment
/// variables.
/// For more information about the formats accepted see `Client::from_config`,
/// and `ClientOptions`.
///
/// # Panics
///
/// This will panic when the provided DSN is invalid.
/// If you want to handle invalid DSNs you need to parse them manually by
/// calling `parse` on it and handle the error.
pub fn init<C>(opts: C) -> ClientInitGuard
where
    C: Into<ClientOptions>,
{
    let opts = apply_defaults(opts.into());
    let client = Arc::new(Client::from(opts));

    Hub::with(|hub| hub.bind_client(Some(client.clone())));
    if let Some(dsn) = client.dsn() {
        sentry_debug!("enabled sentry client for DSN {}", dsn);
    } else {
        sentry_debug!("initialized disabled sentry client due to disabled or invalid DSN");
    }
    ClientInitGuard(client)
}
