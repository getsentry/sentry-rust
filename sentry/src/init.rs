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
        if self.0.is_enabled() {
            sentry_debug!("[ClientInitGuard] Dropping client guard -> disposing client");
            if self.0.close(None) {
                sentry_debug!("[ClientInitGuard] Client closed successfully");
            } else {
                sentry_debug!("[ClientInitGuard] Client close timed out or failed");
            }
        } else {
            sentry_debug!("[ClientInitGuard] Dropping client guard (no client to dispose)");
        }
    }
}

/// Initialize the sentry SDK.
///
/// This function takes any type that implements `Into<ClientOptions>` and
/// initializes the client from it.  Normally you would want to pass a
/// `ClientOptions` but due to this you can also pass a DSN string or other
/// supported config types there.
///
/// This returns a guard that when dropped will close the client.  You can
/// also pass it to `shutdown` to explicitly close the client.
///
/// When no config is provided or an empty DSN is provided the client will
/// be disabled and all operations will become NOPs.
///
/// # Examples
///
/// ```
/// let _guard = sentry::init("https://key@sentry.io/42");
/// ```
///
/// Or provide a more detailed configuration:
///
/// ```
/// let _guard = sentry::init(sentry::ClientOptions {
///     dsn: Some("https://key@sentry.io/42".parse().unwrap()),
///     release: sentry::release_name!(),
///     ..Default::default()
/// });
/// ```
pub fn init<C: Into<ClientOptions>>(cfg: C) -> ClientInitGuard {
    let options = cfg.into();
    sentry_debug!("[init] Initializing Sentry SDK");
    
    // Log key configuration parameters
    if let Some(ref dsn) = options.dsn {
        sentry_debug!("[init] DSN: {}", dsn);
    } else {
        sentry_debug!("[init] No DSN provided - Sentry will be disabled");
    }
    
    sentry_debug!("[init] Debug mode: {}", options.debug);
    sentry_debug!("[init] Sample rate: {}", options.sample_rate);
    sentry_debug!("[init] Traces sample rate: {}", options.traces_sample_rate);
    
    if let Some(ref release) = options.release {
        sentry_debug!("[init] Release: {}", release);
    }
    
    if let Some(ref environment) = options.environment {
        sentry_debug!("[init] Environment: {}", environment);
    }
    
    if let Some(ref server_name) = options.server_name {
        sentry_debug!("[init] Server name: {}", server_name);
    }
    
    sentry_debug!("[init] Default integrations enabled: {}", options.default_integrations);
    sentry_debug!("[init] Custom integrations count: {}", options.integrations.len());
    
    #[cfg(feature = "logs")]
    sentry_debug!("[init] Logs enabled: {}", options.enable_logs);

    // Capture values we need after applying defaults
    #[cfg(feature = "release-health")]
    let auto_session_tracking = options.auto_session_tracking;
    #[cfg(feature = "release-health")]
    let session_mode = options.session_mode;
    
    let client = Arc::new(crate::apply_defaults(options).into());
    
    if client.is_enabled() {
        if let Some(dsn) = client.dsn() {
            sentry_debug!("[init] Enabled sentry client for DSN {}", dsn);
        }
    } else {
        sentry_debug!("[init] Initialized disabled sentry client due to disabled or invalid DSN");
    }
    
    Hub::with(|hub| {
        hub.bind_client(Some(client.clone()));
        sentry_debug!("[init] Bound client to current hub");
    });
    
    #[cfg(feature = "release-health")]
    if auto_session_tracking && session_mode == SessionMode::Application {
        sentry_debug!("[init] Starting automatic session tracking");
        crate::start_session();
    }
    
    sentry_debug!("[init] Sentry SDK initialization complete");
    
    ClientInitGuard(client)
}
