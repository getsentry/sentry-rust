use std::sync::Arc;

use sentry_core::sentry_debug;
#[cfg(feature = "release-health")]
use sentry_core::SessionMode;

use crate::defaults::apply_defaults;
use crate::{Client, ClientOptions, Hub};

/// Helper struct that is returned from `init`.
///
/// When this is dropped events are drained with the configured `shutdown_timeout`.
#[must_use = "when the init guard is dropped the send queue is flushed and the \
              transport will be shut down and no further events can be sent."]
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
        // end any session that might be open before closing the client
        #[cfg(feature = "release-health")]
        crate::end_session();
        self.0.close(None);
    }
}

/// Creates the Sentry client for a given client config and binds it.
///
/// This is the primary way to initialize Sentry in your application. It creates a Sentry client
/// based on the provided configuration, sets up default integrations and transport, and binds
/// the client to the current thread's [`Hub`].
///
/// The function returns a [`ClientInitGuard`] that must be kept in scope. When this guard is
/// dropped, it will flush any pending events and shut down the transport. This ensures that
/// events are sent before your application terminates.
///
/// # Supported Configuration Types
///
/// This function accepts various configuration types:
/// 
/// - **DSN string**: `"https://key@sentry.io/project-id"` 
/// - **Empty/disabled**: `""`, `()`, or `None` to disable Sentry
/// - **[`ClientOptions`]**: Full configuration object for advanced setup
/// - **Tuple**: `(dsn, ClientOptions)` to combine DSN with options
///
/// # Environment Variables
///
/// The following environment variables are automatically read:
/// - `SENTRY_DSN`: Used as default DSN if not provided in config
/// - `SENTRY_ENVIRONMENT`: Sets the environment (e.g., "production", "staging")
/// - `SENTRY_RELEASE`: Sets the release identifier
///
/// # Examples
///
/// ## Basic setup with DSN
/// ```
/// let _guard = sentry::init("https://key@sentry.io/project-id");
/// sentry::capture_message("Hello Sentry!", sentry::Level::Info);
/// ```
///
/// ## Advanced configuration
/// ```
/// let _guard = sentry::init(sentry::ClientOptions {
///     dsn: "https://key@sentry.io/project-id".parse().ok(),
///     release: Some("my-app@1.0.0".into()),
///     environment: Some("production".into()),
///     sample_rate: 0.5, // Sample 50% of events
///     traces_sample_rate: 0.1, // Sample 10% of performance traces
///     attach_stacktrace: true,
///     send_default_pii: false,
///     max_breadcrumbs: 50,
///     ..Default::default()
/// });
/// ```
///
/// ## Disable Sentry (for development/testing)
/// ```
/// let _guard = sentry::init(sentry::ClientOptions::default()); // No DSN = disabled
/// // or
/// let _guard = sentry::init(""); // Empty DSN = disabled
/// ```
///
/// ## Configure with custom integrations
/// ```
/// let _guard = sentry::init(sentry::ClientOptions {
///     dsn: "https://key@sentry.io/project-id".parse().ok(),
///     default_integrations: false, // Disable default integrations
///     ..Default::default()
/// }.add_integration(sentry::integrations::panic::PanicIntegration::new()));
/// ```
///
/// ## Long-running applications
/// ```
/// let guard = sentry::init("https://key@sentry.io/project-id");
/// 
/// // Your application logic here...
/// 
/// // Explicitly flush and shutdown (optional - guard drop will do this too)
/// drop(guard); // or std::mem::drop(guard);
/// ```
///
/// ## Don't wait for shutdown (not recommended)
/// ```
/// std::mem::forget(sentry::init("https://key@sentry.io/project-id"));
/// // Events may be lost if the application terminates quickly
/// ```
///
/// # Return Value
///
/// This returns a guard that when dropped will help the
/// client send events before the application closes. When the guard is
/// dropped, then the transport that was initialized shuts down and no
/// further events can be sent on it.
///
/// If you don't want (or can not) keep the guard around, it's permissible to
/// call `mem::forget` on it.
///
/// # Panics
///
/// This will panic when the provided DSN is invalid.
/// If you want to handle invalid DSNs you need to parse them manually by
/// calling `parse` on each of them and handle the error.
///
/// # Integration Setup
///
/// This behaves similar to creating a client by calling `Client::from_config`
/// and to then bind it to the hub except it also applies default integrations,
/// a default transport, as well as other options populated from environment
/// variables.
/// For more information about the formats accepted see `Client::from_config`,
/// and `ClientOptions`.
pub fn init<C>(opts: C) -> ClientInitGuard
where
    C: Into<ClientOptions>,
{
    let opts = apply_defaults(opts.into());

    #[cfg(feature = "release-health")]
    let auto_session_tracking = opts.auto_session_tracking;
    #[cfg(feature = "release-health")]
    let session_mode = opts.session_mode;

    let client = Arc::new(Client::from(opts));

    Hub::with(|hub| hub.bind_client(Some(client.clone())));
    if let Some(dsn) = client.dsn() {
        sentry_debug!("enabled sentry client for DSN {}", dsn);
    } else {
        sentry_debug!("initialized disabled sentry client due to disabled or invalid DSN");
    }
    #[cfg(feature = "release-health")]
    if auto_session_tracking && session_mode == SessionMode::Application {
        crate::start_session()
    }
    ClientInitGuard(client)
}
