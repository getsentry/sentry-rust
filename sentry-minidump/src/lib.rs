//! Captures native crashes as minidumps in a separate process and sends
//! them to Sentry as attachments.
//!
//! Add [`MinidumpIntegration`] to your [`ClientOptions`]. The integration
//! does all of its work inside `sentry::init`:
//!
//! - In the app process it spawns a crash reporter process and keeps the
//!   handle for the life of the client.
//! - In the crash reporter process it never returns. It builds its own
//!   client from the same options, runs the minidump server, and exits.
//!
//! ```rust
//! let _guard = sentry::init(
//!     sentry::ClientOptions::new()
//!         .dsn("https://your-dsn@sentry.io/0")
//!         .add_integration(
//!             sentry_minidump::MinidumpIntegration::new()
//!                 .crashes_dir("/var/lib/my-app/crashes"),
//!         ),
//! );
//! // Only the app process reaches here.
//! ```
//!
//! Code before `sentry::init` runs in both processes, because the crash
//! reporter re-executes the current binary. Build the integration and call
//! [`MinidumpIntegration::is_crash_reporter_process`] on it to skip work
//! that should run only in the app process.
//!
//! # Scope sync
//!
//! Scope changes do not cross the process boundary on their own. Send them
//! to the crash reporter through the integration:
//!
//! ```rust
//! # let user = sentry::User::default();
//! sentry::with_integration(|minidump: &sentry_minidump::MinidumpIntegration, _| {
//!     minidump.set_user(Some(user.clone()));
//! });
//! ```
//!
//! # Platforms
//!
//! `sentry-minidump` builds on Linux, macOS and Windows only.

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![deny(unsafe_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use minidumper_child::{ClientHandle, MinidumperChild};
use sentry_core::protocol::{Attachment, AttachmentType, Breadcrumb, Event, User, Value};
use sentry_core::{sentry_debug, Client, ClientOptions, Hub, Integration, Level, Scope};

/// The default environment variable that marks the crash reporter process.
const DEFAULT_SERVER_ENV_VAR: &str = "_SENTRY_CRASH_REPORTER_SERVER";

/// The default time to wait for the crash event to upload.
const DEFAULT_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// The default `argv[0]` of the crash reporter process, shown by `ps`.
const DEFAULT_PROCESS_NAME: &str = "Crash Reporter (Sentry Rust SDK)";

type OnProcess = Box<dyn FnOnce(&mut Command) + Send + Sync + 'static>;
type BeforeCapture = dyn Fn(&mut Scope, &Path) + Send + Sync + 'static;

/// An update to the crash reporter's scope, sent over the socket.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
enum ScopeUpdate {
    AddBreadcrumb(Breadcrumb),
    SetUser(Option<User>),
    SetExtra(String, Option<Value>),
    SetTag(String, Option<String>),
}

/// Captures native crashes as minidumps and sends them to Sentry.
///
/// Build it with [`MinidumpIntegration::new`] and the builder methods, then
/// pass it to [`ClientOptions::add_integration`]. See the
/// [crate docs](crate) for the process model.
pub struct MinidumpIntegration {
    crashes_dir: Option<PathBuf>,
    server_env_var: String,
    inherit_args: bool,
    process_name: Option<OsString>,
    on_process: Mutex<Option<OnProcess>>,
    before_capture: Option<Arc<BeforeCapture>>,
    flush_timeout: Duration,
    client_connect_timeout: Option<Duration>,
    server_stale_timeout: Option<Duration>,
    handle: OnceLock<ClientHandle>,
}

impl std::fmt::Debug for MinidumpIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinidumpIntegration")
            .field("crashes_dir", &self.crashes_dir)
            .field("server_env_var", &self.server_env_var)
            .field("inherit_args", &self.inherit_args)
            .field("process_name", &self.process_name)
            .field("flush_timeout", &self.flush_timeout)
            .field("client_connect_timeout", &self.client_connect_timeout)
            .field("server_stale_timeout", &self.server_stale_timeout)
            .finish_non_exhaustive()
    }
}

impl Default for MinidumpIntegration {
    fn default() -> Self {
        Self {
            crashes_dir: None,
            server_env_var: DEFAULT_SERVER_ENV_VAR.to_owned(),
            inherit_args: true,
            process_name: Some(OsString::from(DEFAULT_PROCESS_NAME)),
            on_process: Mutex::new(None),
            before_capture: None,
            flush_timeout: DEFAULT_FLUSH_TIMEOUT,
            client_connect_timeout: None,
            server_stale_timeout: None,
            handle: OnceLock::new(),
        }
    }
}

impl MinidumpIntegration {
    /// Creates a new minidump integration with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the current process is the crash reporter process.
    ///
    /// This respects [`server_env_var`](Self::server_env_var) and does not
    /// need a Sentry client, so it can run before logging or Sentry are set
    /// up. Build the integration, then call this on it to skip app-only
    /// work in the crash reporter process.
    pub fn is_crash_reporter_process(&self) -> bool {
        std::env::var_os(&self.server_env_var).is_some()
    }

    /// Sets the directory where minidumps are written before upload.
    ///
    /// Defaults to `Crashes` inside the system temp directory.
    #[must_use]
    pub fn crashes_dir<P>(mut self, dir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.crashes_dir = Some(dir.into());
        self
    }

    /// Sets the environment variable that marks the crash reporter process.
    ///
    /// Defaults to `_SENTRY_CRASH_REPORTER_SERVER`. Change it when the
    /// default could clash with another crash reporter in the same process
    /// tree.
    #[must_use]
    pub fn server_env_var<S>(mut self, name: S) -> Self
    where
        S: Into<String>,
    {
        self.server_env_var = name.into();
        self
    }

    /// Passes the arguments of the current process to the crash reporter
    /// process.
    ///
    /// Useful if the app reads its config from command-line arguments.
    /// Defaults to `true`.
    #[must_use]
    pub fn inherit_args(mut self, inherit: bool) -> Self {
        self.inherit_args = inherit;
        self
    }

    /// Sets the process name of the crash reporter as shown by `ps`.
    ///
    /// This sets `argv[0]` on unix. It has no effect on other platforms.
    /// Defaults to `Crash Reporter (Sentry Rust SDK)`.
    #[must_use]
    pub fn process_name<S>(mut self, name: S) -> Self
    where
        S: Into<OsString>,
    {
        self.process_name = Some(name.into());
        self
    }

    /// Modifies the [`Command`] used to spawn the crash reporter process.
    ///
    /// Runs after [`inherit_args`](Self::inherit_args) and
    /// [`process_name`](Self::process_name) are applied. The marker
    /// environment variable is set after this callback, so `env_clear`
    /// does not break process detection.
    #[must_use]
    pub fn on_process<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut Command) + Send + Sync + 'static,
    {
        self.on_process = Mutex::new(Some(Box::new(f)));
        self
    }

    /// Runs in the crash reporter process before the crash event is sent.
    ///
    /// The scope already holds the minidump attachment. Use this to add
    /// tags or to log the path of the minidump.
    #[must_use]
    pub fn before_capture<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Scope, &Path) + Send + Sync + 'static,
    {
        self.before_capture = Some(Arc::new(f));
        self
    }

    /// Sets how long the crash reporter waits for the upload to finish.
    ///
    /// Defaults to 5 seconds.
    #[must_use]
    pub fn flush_timeout(mut self, timeout: Duration) -> Self {
        self.flush_timeout = timeout;
        self
    }

    /// Sets how long the app waits for the crash reporter to accept a
    /// connection.
    #[must_use]
    pub fn client_connect_timeout(mut self, timeout: Duration) -> Self {
        self.client_connect_timeout = Some(timeout);
        self
    }

    /// Sets how long the crash reporter waits without a ping from the app
    /// before it exits.
    #[must_use]
    pub fn server_stale_timeout(mut self, timeout: Duration) -> Self {
        self.server_stale_timeout = Some(timeout);
        self
    }

    /// Sends a scope update to the crash reporter process.
    ///
    /// Does nothing when there is no crash reporter, because this is the
    /// reporter process, the DSN was unset, or the spawn failed.
    fn send(&self, update: &ScopeUpdate) {
        if let Some(handle) = self.handle.get() {
            if let Ok(buffer) = serde_json::to_vec(update) {
                handle.send_message(0, buffer).ok();
            }
        }
    }

    /// Adds a breadcrumb to the crash reporter's scope.
    pub fn add_breadcrumb(&self, breadcrumb: Breadcrumb) {
        self.send(&ScopeUpdate::AddBreadcrumb(breadcrumb));
    }

    /// Sets the user on the crash reporter's scope.
    pub fn set_user(&self, user: Option<User>) {
        self.send(&ScopeUpdate::SetUser(user));
    }

    /// Sets or removes an extra value on the crash reporter's scope.
    pub fn set_extra(&self, key: String, value: Option<Value>) {
        self.send(&ScopeUpdate::SetExtra(key, value));
    }

    /// Sets or removes a tag on the crash reporter's scope.
    pub fn set_tag(&self, key: String, value: Option<String>) {
        self.send(&ScopeUpdate::SetTag(key, value));
    }

    /// Builds the child from the stored settings.
    fn build_child(&self) -> MinidumperChild {
        let mut child = MinidumperChild::new().with_server_env_var(self.server_env_var.clone());

        if let Some(dir) = &self.crashes_dir {
            child = child.with_crashes_dir(dir.clone());
        }
        if let Some(timeout) = self.client_connect_timeout {
            child = child.with_client_connect_timeout(timeout);
        }
        if let Some(timeout) = self.server_stale_timeout {
            child = child.with_server_stale_timeout(timeout);
        }

        let inherit_args = self.inherit_args;
        let process_name = self.process_name.clone();
        let on_process = self
            .on_process
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        child = child.on_process(move |command| {
            if inherit_args {
                command.args(std::env::args_os().skip(1));
            }

            #[cfg(unix)]
            if let Some(name) = process_name {
                use std::os::unix::process::CommandExt;
                command.arg0(name);
            }
            #[cfg(not(unix))]
            let _ = process_name;

            if let Some(on_process) = on_process {
                on_process(command);
            }
        });

        child = child.on_message(|_kind, buffer| {
            if let Ok(update) = serde_json::from_slice::<ScopeUpdate>(&buffer) {
                let hub = Hub::current();
                match update {
                    ScopeUpdate::AddBreadcrumb(b) => hub.add_breadcrumb(b),
                    ScopeUpdate::SetUser(u) => hub.configure_scope(|scope| scope.set_user(u)),
                    ScopeUpdate::SetExtra(k, v) => hub.configure_scope(|scope| match v {
                        Some(v) => scope.set_extra(&k, v),
                        None => scope.remove_extra(&k),
                    }),
                    ScopeUpdate::SetTag(k, v) => hub.configure_scope(|scope| match v {
                        Some(v) => scope.set_tag(&k, &v),
                        None => scope.remove_tag(&k),
                    }),
                }
            }
        });

        let flush_timeout = self.flush_timeout;
        let before_capture = self.before_capture.clone();
        child.on_minidump(move |buffer, path| {
            // The client lives on the current hub, bound in the reporter
            // process before the server starts. Resolve it at crash time
            // rather than capturing it here.
            let hub = Hub::current();
            hub.with_scope(
                |scope| {
                    // This event came from the app process, not the crash
                    // reporter, so drop the reporter marker.
                    scope.remove_extra("event.process");

                    let filename = path
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "minidump.dmp".to_string());

                    scope.add_attachment(Attachment {
                        buffer,
                        filename,
                        ty: Some(AttachmentType::Minidump),
                        ..Default::default()
                    });

                    if let Some(before_capture) = &before_capture {
                        before_capture(scope, path);
                    }
                },
                || {
                    hub.capture_event(Event {
                        level: Level::Fatal,
                        ..Default::default()
                    });
                },
            );

            // The server exits after this closure returns, so flush now.
            if let Some(client) = hub.client() {
                client.flush(Some(flush_timeout));
            }
        })
    }
}

/// Runs the crash reporter process and never returns.
///
/// Builds a second client from the app's options, binds it to the current
/// hub, then runs the minidump server. `spawn` exits the process with 0 in
/// server mode. Any error means the reporter could not start, so capture it
/// and exit with 1 rather than fall through into app code.
fn run_crash_reporter(
    child: MinidumperChild,
    options: &ClientOptions,
    flush_timeout: Duration,
) -> ! {
    let mut reporter_options = options.clone();
    // Drop this integration from the second client so its setup does not
    // recurse into the reporter path.
    reporter_options.integrations.retain(|i| {
        // `as_ref` first, so `as_any` dispatches through the
        // `dyn Integration` vtable. Calling `as_any` on the `Arc`
        // directly would downcast the wrapper and never match.
        i.as_ref()
            .as_any()
            .downcast_ref::<MinidumpIntegration>()
            .is_none()
    });

    let client = Client::with_options(reporter_options);
    let hub = Hub::current();
    hub.bind_client(Some(Arc::new(client)));

    // Mark Rust events so it is obvious when they come from the crash
    // reporter rather than the app.
    hub.configure_scope(|scope| {
        scope.set_extra("event.process", Value::String("crash-reporter".to_string()));
    });

    match child.spawn() {
        // `spawn` exits the process itself in server mode, so this arm is
        // only a safety net.
        Ok(_handle) => exit(0),
        Err(err) => {
            hub.capture_error(&err);
            if let Some(client) = hub.client() {
                client.flush(Some(flush_timeout));
            }
            exit(1);
        }
    }
}

impl Integration for MinidumpIntegration {
    fn name(&self) -> &'static str {
        "minidump"
    }

    fn setup(&self, options: &mut ClientOptions) {
        let child = self.build_child();

        if self.is_crash_reporter_process() {
            run_crash_reporter(child, options, self.flush_timeout);
        }

        // App process. Without a DSN there is nothing to report, so do not
        // spawn a process.
        if options.dsn.is_none() {
            return;
        }

        // Spawn once per integration instance, guarded by `handle`. A second
        // `sentry::init` uses a new instance with an empty handle, so
        // re-initialising the SDK starts a fresh reporter.
        if self.handle.get().is_some() {
            return;
        }
        match child.spawn() {
            Ok(handle) => {
                let _ = self.handle.set(handle);
            }
            Err(err) => {
                sentry_debug!("could not start crash reporter: {err}");
            }
        }
    }
}
