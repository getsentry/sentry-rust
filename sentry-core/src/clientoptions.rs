#![allow(deprecated)]

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::constants::USER_AGENT;
use crate::protocol::{Breadcrumb, Event};
use crate::types::Dsn;
use crate::{Integration, IntoDsn, TransportFactory};

/// Type alias for before event/breadcrumb handlers.
pub type BeforeCallback<T> = Arc<dyn Fn(T) -> Option<T> + Send + Sync>;

/// Configuration settings for the client.
///
/// These options are explained in more detail in the general
/// [sentry documentation](https://docs.sentry.io/error-reporting/configuration/?platform=rust).
///
/// # Examples
///
/// ```
/// let _options = sentry::ClientOptions {
///     debug: true,
///     ..Default::default()
/// };
/// ```
#[derive(Clone)]
pub struct ClientOptions {
    // Common options
    /// The DSN to use.  If not set the client is effectively disabled.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub dsn: Option<Dsn>,
    /// Enables debug mode.
    ///
    /// In debug mode debug information is printed to stderr to help you understand what
    /// sentry is doing.  When the `log` feature is enabled, Sentry will instead
    /// log to the `sentry` logger independently of this flag with the `Debug` level.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub debug: bool,
    /// The release to be sent with events.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub release: Option<Cow<'static, str>>,
    /// The environment to be sent with events.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub environment: Option<Cow<'static, str>>,
    /// The sample rate for event submission. (0.0 - 1.0, defaults to 1.0)
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub sample_rate: f32,
    /// Maximum number of breadcrumbs. (defaults to 100)
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub max_breadcrumbs: usize,
    /// Attaches stacktraces to messages.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub attach_stacktrace: bool,
    /// If turned on some default PII informat is attached.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub send_default_pii: bool,
    /// The server name to be reported.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub server_name: Option<Cow<'static, str>>,
    /// Module prefixes that are always considered "in_app".
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub in_app_include: Vec<&'static str>,
    /// Module prefixes that are never "in_app".
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub in_app_exclude: Vec<&'static str>,
    // Integration options
    /// A list of integrations to enable.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub integrations: Vec<Arc<dyn Integration>>,
    /// Whether to add default integrations.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub default_integrations: bool,
    // Hooks
    /// Callback that is executed before event sending.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub before_send: Option<BeforeCallback<Event<'static>>>,
    /// Callback that is executed for each Breadcrumb being added.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub before_breadcrumb: Option<BeforeCallback<Breadcrumb>>,
    // Transport options
    /// The transport to use.
    ///
    /// This is typically either a boxed function taking the client options by
    /// reference and returning a `Transport`, a boxed `Arc<Transport>` or
    /// alternatively the `DefaultTransportFactory`.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub transport: Option<Arc<dyn TransportFactory>>,
    /// An optional HTTP proxy to use.
    ///
    /// This will default to the `http_proxy` environment variable.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub http_proxy: Option<Cow<'static, str>>,
    /// An optional HTTPS proxy to use.
    ///
    /// This will default to the `HTTPS_PROXY` environment variable
    /// or `http_proxy` if that one exists.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub https_proxy: Option<Cow<'static, str>>,
    /// The timeout on client drop for draining events on shutdown.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub shutdown_timeout: Duration,
    // Other options not documented in Unified API
    /// Enable Release Health Session tracking.
    ///
    /// When automatic session tracking is enabled, a new "user-mode" session
    /// is started at the time of `sentry::init`, and will persist for the
    /// application lifetime.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub auto_session_tracking: bool,
    /// Border frames which indicate a border from a backtrace to
    /// useless internals. Some are automatically included.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub extra_border_frames: Vec<&'static str>,
    /// Automatically trim backtraces of junk before sending. (defaults to true)
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub trim_backtraces: bool,
    /// The user agent that should be reported.
    #[deprecated = "use accessor functions instead; direct field access will be removed soon"]
    pub user_agent: Cow<'static, str>,
}

impl ClientOptions {
    /// Creates new Options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates new Options and immediately configures them.
    pub fn configure<F>(f: F) -> Self
    where
        F: FnOnce(&mut ClientOptions) -> &mut ClientOptions,
    {
        let mut opts = Self::new();
        f(&mut opts);
        opts
    }

    /// Set a DSN to use.
    ///
    /// If not set the client is effectively disabled.
    pub fn set_dsn(&mut self, dsn: Dsn) -> &mut Self {
        self.dsn = Some(dsn);
        self
    }
    /// The configured DSN.
    pub fn dsn(&self) -> Option<&Dsn> {
        self.dsn.as_ref()
    }

    /// Enables/disables debug mode.
    ///
    /// In debug mode debug information is printed to stderr to help you understand what
    /// sentry is doing.  When the `log` feature is enabled, Sentry will instead
    /// log to the `sentry` logger independently of this flag with the `Debug` level.
    pub fn set_debug(&mut self, debug: bool) -> &mut Self {
        self.debug = debug;
        self
    }
    /// Whether debug logging is enabled.
    pub fn debug(&self) -> bool {
        self.debug
    }

    /// Set the release to be sent with events.
    pub fn set_release(&mut self, release: Option<Cow<'static, str>>) -> &mut Self {
        self.release = release;
        self
    }
    /// The release to be sent with events.
    pub fn release(&self) -> Option<Cow<'static, str>> {
        self.release.clone()
    }

    /// Set the environment to be sent with events.
    pub fn set_environment(&mut self, environment: Option<Cow<'static, str>>) -> &mut Self {
        self.environment = environment;
        self
    }
    /// The environment to be sent with events.
    pub fn environment(&self) -> Option<Cow<'static, str>> {
        self.environment.clone()
    }

    /// Set the sample rate for event submission. (0.0 - 1.0, defaults to 1.0)
    pub fn set_sample_rate(&mut self, sample_rate: f32) -> &mut Self {
        self.sample_rate = sample_rate;
        self
    }
    /// The sample rate for event submission.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Set the maximum number of breadcrumbs. (defaults to 100)
    pub fn set_max_breadcrumbs(&mut self, max_breadcrumbs: usize) -> &mut Self {
        self.max_breadcrumbs = max_breadcrumbs;
        self
    }
    /// Maximum number of breadcrumbs.
    pub fn max_breadcrumbs(&self) -> usize {
        self.max_breadcrumbs
    }

    /// Enable attaching stacktraces to message events.
    pub fn set_attach_stacktrace(&mut self, attach_stacktrace: bool) -> &mut Self {
        self.attach_stacktrace = attach_stacktrace;
        self
    }
    /// Attach stacktraces to message events.
    pub fn attach_stacktrace(&self) -> bool {
        self.attach_stacktrace
    }

    /// Attach some default PII informat to events.
    pub fn set_send_default_pii(&mut self, send_default_pii: bool) -> &mut Self {
        self.send_default_pii = send_default_pii;
        self
    }
    /// If turned on some default PII informat is attached to events.
    pub fn send_default_pii(&self) -> bool {
        self.send_default_pii
    }

    /// Set the server name to be reported.
    pub fn set_server_name(&mut self, server_name: Option<Cow<'static, str>>) -> &mut Self {
        self.server_name = server_name;
        self
    }
    /// The server name to be reported.
    pub fn server_name(&self) -> Option<Cow<'static, str>> {
        self.server_name.clone()
    }

    /// Add module prefixes that are always considered "in_app".
    pub fn add_in_app_include(&mut self, in_app_include: &[&'static str]) -> &mut Self {
        self.in_app_include.extend_from_slice(in_app_include);
        self
    }
    /// Module prefixes that are always considered "in_app".
    pub fn in_app_include(&self) -> &[&'static str] {
        &self.in_app_include
    }

    /// Add module prefixes that are never "in_app".
    pub fn add_in_app_exclude(&mut self, in_app_exclude: &[&'static str]) -> &mut Self {
        self.in_app_exclude.extend_from_slice(in_app_exclude);
        self
    }
    /// Module prefixes that are never "in_app".
    pub fn in_app_exclude(&self) -> &[&'static str] {
        &self.in_app_exclude
    }

    /// Enable adding default integrations on init.
    pub fn set_default_integrations(&mut self, default_integrations: bool) -> &mut Self {
        self.default_integrations = default_integrations;
        self
    }
    /// Whether to add default integrations.
    pub fn default_integrations(&self) -> bool {
        self.default_integrations
    }

    /// Adds another integration *in front* of the already registered ones.
    // pub fn unshift_integration<I: Integration>(&mut self, integration: I) -> &mut Self {
    //     self.integrations.push_front(Arc::new(integration));
    //     self
    // }

    /// Set a callback that is executed before event sending.
    pub fn set_before_send<F>(&mut self, before_send: F) -> &mut Self
    where
        F: Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        self.before_send = Some(Arc::new(before_send));
        self
    }

    /// Set a callback that is executed for each Breadcrumb being added.
    pub fn set_before_breadcrumb<F>(&mut self, before_breadcrumb: F) -> &mut Self
    where
        F: Fn(Breadcrumb) -> Option<Breadcrumb> + Send + Sync + 'static,
    {
        self.before_breadcrumb = Some(Arc::new(before_breadcrumb));
        self
    }

    /// The transport to use.
    ///
    /// This is typically either a boxed function taking the client options by
    /// reference and returning a `Transport`, a boxed `Arc<Transport>` or
    /// alternatively the `DefaultTransportFactory`.
    pub fn set_transport<F>(&mut self, transport: F) -> &mut Self
    where
        F: TransportFactory + 'static,
    {
        self.transport = Some(Arc::new(transport));
        self
    }
    /// Whether a [`TransportFactory`] has been set on these options.
    pub fn has_transport(&self) -> bool {
        self.transport.is_some()
    }

    /// An optional HTTP proxy to use.
    ///
    /// This will default to the `http_proxy` environment variable.
    pub fn set_http_proxy(&mut self, http_proxy: Option<Cow<'static, str>>) -> &mut Self {
        self.http_proxy = http_proxy;
        self
    }
    /// The HTTP proxy Sentry will use.
    pub fn http_proxy(&self) -> Option<Cow<'static, str>> {
        self.http_proxy.clone()
    }

    /// Set an optional HTTPS proxy to use.
    ///
    /// This will default to the `HTTPS_PROXY` environment variable
    /// or `http_proxy` if that one exists.
    pub fn set_https_proxy(&mut self, https_proxy: Option<Cow<'static, str>>) -> &mut Self {
        self.https_proxy = https_proxy;
        self
    }
    /// The HTTPS proxy Sentry will use.
    pub fn https_proxy(&self) -> Option<Cow<'static, str>> {
        self.https_proxy.clone()
    }

    /// The timeout on client drop for draining events on shutdown.
    pub fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    /// Enable Release Health Session tracking.
    ///
    /// When automatic session tracking is enabled, a new "user-mode" session
    /// is started at the time of `sentry::init`, and will persist for the
    /// application lifetime.
    pub fn set_auto_session_tracking(&mut self, auto_session_tracking: bool) -> &mut Self {
        self.auto_session_tracking = auto_session_tracking;
        self
    }
    /// Whether automatic session tracking is enabled.
    pub fn auto_session_tracking(&self) -> bool {
        self.auto_session_tracking
    }

    /// Add extra border frames which indicate a border from a backtrace to
    /// useless internals.
    pub fn add_extra_border_frames(&mut self, extra_border_frames: &[&'static str]) -> &mut Self {
        self.extra_border_frames
            .extend_from_slice(extra_border_frames);
        self
    }
    /// Border frames which indicate a border from a backtrace to
    /// useless internals. Some are automatically included.
    pub fn extra_border_frames(&self) -> &[&'static str] {
        &self.extra_border_frames
    }

    /// Automatically trim backtraces of junk before sending. (defaults to true)
    pub fn set_trim_backtraces(&mut self, trim_backtraces: bool) -> &mut Self {
        self.trim_backtraces = trim_backtraces;
        self
    }
    /// Automatically trim backtraces of junk before sending.
    pub fn trim_backtraces(&self) -> bool {
        self.trim_backtraces
    }

    /// Set the user agent that should be reported.
    pub fn set_user_agent(&mut self, user_agent: Cow<'static, str>) -> &mut Self {
        self.user_agent = user_agent;
        self
    }
    /// The user agent that should be reported.
    pub fn user_agent(&self) -> Cow<'static, str> {
        self.user_agent.clone()
    }

    /// Adds a configured integration to the options.
    ///
    /// # Examples
    ///
    /// ```
    /// struct MyIntegration;
    ///
    /// impl sentry::Integration for MyIntegration {}
    ///
    /// let options = sentry::ClientOptions::new().add_integration(MyIntegration);
    /// assert_eq!(options.integrations.len(), 1);
    /// ```
    pub fn add_integration<I: Integration>(mut self, integration: I) -> Self {
        self.integrations.push(Arc::new(integration));
        self
    }
}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct BeforeSend;
        let before_send = self.before_send.as_ref().map(|_| BeforeSend);
        #[derive(Debug)]
        struct BeforeBreadcrumb;
        let before_breadcrumb = self.before_breadcrumb.as_ref().map(|_| BeforeBreadcrumb);
        #[derive(Debug)]
        struct TransportFactory;

        let integrations: Vec<_> = self.integrations.iter().map(|i| i.name()).collect();

        f.debug_struct("ClientOptions")
            .field("dsn", &self.dsn)
            .field("debug", &self.debug)
            .field("release", &self.release)
            .field("environment", &self.environment)
            .field("sample_rate", &self.sample_rate)
            .field("max_breadcrumbs", &self.max_breadcrumbs)
            .field("attach_stacktrace", &self.attach_stacktrace)
            .field("send_default_pii", &self.send_default_pii)
            .field("server_name", &self.server_name)
            .field("in_app_include", &self.in_app_include)
            .field("in_app_exclude", &self.in_app_exclude)
            .field("integrations", &integrations)
            .field("default_integrations", &self.default_integrations)
            .field("before_send", &before_send)
            .field("before_breadcrumb", &before_breadcrumb)
            .field("transport", &TransportFactory)
            .field("http_proxy", &self.http_proxy)
            .field("https_proxy", &self.https_proxy)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("auto_session_tracking", &self.auto_session_tracking)
            .field("extra_border_frames", &self.extra_border_frames)
            .field("trim_backtraces", &self.trim_backtraces)
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl Default for ClientOptions {
    fn default() -> ClientOptions {
        ClientOptions {
            dsn: None,
            debug: false,
            release: None,
            environment: None,
            sample_rate: 1.0,
            max_breadcrumbs: 100,
            attach_stacktrace: false,
            send_default_pii: false,
            server_name: None,
            in_app_include: vec![],
            in_app_exclude: vec![],
            integrations: vec![],
            default_integrations: true,
            before_send: None,
            before_breadcrumb: None,
            transport: None,
            http_proxy: None,
            https_proxy: None,
            shutdown_timeout: Duration::from_secs(2),
            auto_session_tracking: false,
            extra_border_frames: vec![],
            trim_backtraces: true,
            user_agent: Cow::Borrowed(&USER_AGENT),
        }
    }
}

impl<T: IntoDsn> From<(T, ClientOptions)> for ClientOptions {
    fn from((into_dsn, mut opts): (T, ClientOptions)) -> ClientOptions {
        opts.dsn = into_dsn.into_dsn().expect("invalid value for DSN");
        opts
    }
}

impl<T: IntoDsn> From<T> for ClientOptions {
    fn from(into_dsn: T) -> ClientOptions {
        ClientOptions {
            dsn: into_dsn.into_dsn().expect("invalid value for DSN"),
            ..ClientOptions::default()
        }
    }
}
