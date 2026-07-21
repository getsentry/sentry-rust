use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::constants::USER_AGENT;
use crate::performance::{TracesSampler, TransactionContext};
use crate::protocol::{Breadcrumb, Event, Log, Metric};
use crate::types::Dsn;
use crate::{Integration, IntoDsn, TransportFactory};

/// Type alias for before event/breadcrumb handlers.
pub type BeforeCallback<T> = Arc<dyn Fn(T) -> Option<T> + Send + Sync>;

/// The Session Mode of the SDK.
///
/// Depending on the use-case, the SDK can be set to two different session modes:
///
/// * **Application Mode Sessions**:
///   This mode should be used for user-attended programs, which typically have
///   a single long running session that span the applications' lifetime.
///
/// * **Request Mode Sessions**:
///   This mode is intended for servers that use one session per incoming
///   request, and thus have a lot of very short lived sessions.
///
/// Setting the SDK to *request-mode* sessions means that session durations will
/// not be tracked, and sessions will be pre-aggregated before being sent upstream.
/// This applies both to automatic and manually triggered sessions.
///
/// **NOTE**: Support for *request-mode* sessions was added in Sentry `21.2`.
///
/// See the
/// [Documentation on Session Modes](https://develop.sentry.dev/sdk/sessions/#sdk-considerations)
/// for more information.
///
/// **NOTE**: The `release-health` feature (enabled by default) needs to be enabled for this
/// option to have any effect.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SessionMode {
    /// Long running application session.
    Application,
    /// Lots of short per-request sessions.
    Request,
}

/// The maximum size of an HTTP request body that the SDK captures.
///
/// Only request bodies that parse as JSON or form data are currently captured.
/// See the Sentry documentation on [attaching request bodies] and [handling sensitive data] for
/// more information.
///
/// [attaching request bodies]: https://develop.sentry.dev/sdk/expected-features/#attaching-request-body-in-server-sdks
/// [handling sensitive data]: https://develop.sentry.dev/sdk/expected-features/data-handling/#sensitive-data
#[derive(Clone, Copy, PartialEq)]
pub enum MaxRequestBodySize {
    /// Don't capture request body
    None,
    /// Capture up to 1000 bytes
    Small,
    /// Capture up to 10000 bytes
    Medium,
    /// Capture entire body
    Always,
    /// Capture up to a specific size
    Explicit(usize),
}

impl MaxRequestBodySize {
    /// Check if the content length is within the size limit.
    pub fn is_within_size_limit(&self, content_length: usize) -> bool {
        match self {
            MaxRequestBodySize::None => false,
            MaxRequestBodySize::Small => content_length <= 1_000,
            MaxRequestBodySize::Medium => content_length <= 10_000,
            MaxRequestBodySize::Always => true,
            MaxRequestBodySize::Explicit(size) => content_length <= *size,
        }
    }
}

/// Configuration settings for the client.
///
/// These options are explained in more detail in the general
/// [sentry documentation](https://docs.sentry.io/error-reporting/configuration/?platform=rust).
///
/// # Examples
///
/// ```
/// let _options = sentry::ClientOptions::new().debug(true);
/// ```
#[derive(Clone)]
#[must_use = "ClientOptions must be passed to sentry::init to have any effect"]
#[non_exhaustive]
pub struct ClientOptions {
    // Common options
    /// The DSN to use.
    ///
    /// See [`dsn`](method@ClientOptions::dsn) for details.
    pub dsn: Option<Dsn>,
    /// Enables debug mode.
    ///
    /// See [`debug`](method@ClientOptions::debug) for details.
    pub debug: bool,
    /// The release to be sent with events.
    ///
    /// See [`release`](method@ClientOptions::release) for details.
    pub release: Option<Cow<'static, str>>,
    /// The environment to be sent with events.
    ///
    /// See [`environment`](method@ClientOptions::environment) for details.
    pub environment: Option<Cow<'static, str>>,
    /// The sample rate for event submission.
    ///
    /// See [`sample_rate`](method@ClientOptions::sample_rate) for details.
    pub sample_rate: f32,
    /// The sample rate for tracing transactions.
    ///
    /// See [`traces_sample_rate`](method@ClientOptions::traces_sample_rate) for details.
    pub traces_sample_rate: f32,
    /// The sampler callback for tracing transactions.
    ///
    /// See [`traces_sampler`](method@ClientOptions::traces_sampler) for details.
    pub traces_sampler: Option<Arc<TracesSampler>>,
    /// Maximum number of breadcrumbs.
    ///
    /// See [`max_breadcrumbs`](method@ClientOptions::max_breadcrumbs) for details.
    pub max_breadcrumbs: usize,
    /// Attaches stacktraces to messages.
    ///
    /// See [`attach_stacktrace`](method@ClientOptions::attach_stacktrace) for details.
    pub attach_stacktrace: bool,
    /// Whether to send default PII.
    ///
    /// See [`send_default_pii`](method@ClientOptions::send_default_pii) for details.
    pub send_default_pii: bool,
    /// The server name to be reported.
    ///
    /// See [`server_name`](method@ClientOptions::server_name) for details.
    pub server_name: Option<Cow<'static, str>>,
    /// Module prefixes that are always considered "in_app".
    ///
    /// See [`in_app_include`](method@ClientOptions::in_app_include) for details.
    pub in_app_include: Vec<&'static str>,
    /// Module prefixes that are never "in_app".
    ///
    /// See [`in_app_exclude`](method@ClientOptions::in_app_exclude) for details.
    pub in_app_exclude: Vec<&'static str>,
    // Integration options
    /// A list of integrations to enable.
    ///
    /// See [`integrations`](method@ClientOptions::integrations) and
    /// [`add_integration`](method@ClientOptions::add_integration) for details.
    pub integrations: Vec<Arc<dyn Integration>>,
    /// Whether to add default integrations.
    ///
    /// See [`default_integrations`](method@ClientOptions::default_integrations) for details.
    pub default_integrations: bool,
    // Hooks
    /// Callback that is executed before event sending.
    ///
    /// See [`before_send`](method@ClientOptions::before_send) for details.
    pub before_send: Option<BeforeCallback<Event<'static>>>,
    /// Callback that is executed for each Breadcrumb being added.
    ///
    /// See [`before_breadcrumb`](method@ClientOptions::before_breadcrumb) for details.
    pub before_breadcrumb: Option<BeforeCallback<Breadcrumb>>,
    /// Callback that is executed for each Log being added.
    ///
    /// See [`before_send_log`](method@ClientOptions::before_send_log) for details.
    pub before_send_log: Option<BeforeCallback<Log>>,
    // Transport options
    /// The transport to use.
    ///
    /// See [`transport`](method@ClientOptions::transport) for details.
    pub transport: Option<Arc<dyn TransportFactory>>,
    /// An optional HTTP proxy to use.
    ///
    /// See [`http_proxy`](method@ClientOptions::http_proxy) for details.
    pub http_proxy: Option<Cow<'static, str>>,
    /// An optional HTTPS proxy to use.
    ///
    /// See [`https_proxy`](method@ClientOptions::https_proxy) for details.
    pub https_proxy: Option<Cow<'static, str>>,
    /// The timeout on client drop for draining events on shutdown.
    ///
    /// See [`shutdown_timeout`](method@ClientOptions::shutdown_timeout) for details.
    pub shutdown_timeout: Duration,
    /// The maximum size of an HTTP request body to capture.
    ///
    /// See [`max_request_body_size`](method@ClientOptions::max_request_body_size) for details.
    pub max_request_body_size: MaxRequestBodySize,
    /// Whether captured structured logs should be sent to Sentry.
    ///
    /// See [`enable_logs`](method@ClientOptions::enable_logs) for details.
    pub enable_logs: bool,
    /// Whether metric capture APIs should capture metrics.
    ///
    /// See [`enable_metrics`](method@ClientOptions::enable_metrics) for details.
    pub enable_metrics: bool,
    /// Callback that is executed for each [`Metric`] before sending.
    ///
    /// See [`before_send_metric`](method@ClientOptions::before_send_metric) for details.
    pub before_send_metric: Option<BeforeCallback<Metric>>,
    // Other options not documented in Unified API
    /// Whether to disable SSL verification.
    ///
    /// See [`accept_invalid_certs`](method@ClientOptions::accept_invalid_certs) for details.
    pub accept_invalid_certs: bool,
    /// Whether Release Health Session tracking is enabled.
    ///
    /// See [`auto_session_tracking`](method@ClientOptions::auto_session_tracking) for details.
    pub auto_session_tracking: bool,
    /// Determine how Sessions are being tracked.
    ///
    /// See [`session_mode`](method@ClientOptions::session_mode) for details.
    pub session_mode: SessionMode,
    /// The user agent that should be reported.
    ///
    /// See [`user_agent`](method@ClientOptions::user_agent) for details.
    pub user_agent: Cow<'static, str>,
}

impl ClientOptions {
    /// Creates new Options.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the [DSN](field@ClientOptions::dsn) to use.
    ///
    /// # Panics
    ///
    /// Panics if the value fails to parse as a [DSN](`Dsn`).
    #[inline]
    pub fn dsn(self, dsn: &str) -> Self {
        let dsn = Some(dsn.parse().expect("invalid value for DSN"));
        Self { dsn, ..self }
    }

    /// Enables or disables [debug mode](field@ClientOptions::debug).
    ///
    /// In debug mode debug information is printed to stderr to help you understand what sentry is
    /// doing. Defaults to `false`.
    #[inline]
    pub fn debug(self, debug: bool) -> Self {
        Self { debug, ..self }
    }

    /// Sets the [release](field@ClientOptions::release) to be sent with events.
    #[inline]
    pub fn release<T>(self, release: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        let release = Some(release.into());
        Self { release, ..self }
    }

    /// Sets the [release](field@ClientOptions::release) to be sent with events if one is provided.
    ///
    /// Use this with [`release_name!`](crate::release_name), which returns the release as an
    /// `Option`.
    #[inline]
    pub fn maybe_release<T>(self, release: Option<T>) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        match release {
            Some(release) => self.release(release),
            None => self,
        }
    }

    /// Sets the [environment](field@ClientOptions::environment) to be sent with events.
    ///
    /// Defaults to either `"development"` or `"production"` depending on the `debug_assertions`
    /// cfg-attribute.
    #[inline]
    pub fn environment<T>(self, environment: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        let environment = Some(environment.into());
        Self {
            environment,
            ..self
        }
    }

    /// Sets the [sample rate](field@ClientOptions::sample_rate) for event submission.
    ///
    /// Must be between `0.0` and `1.0`. Defaults to `1.0`.
    ///
    /// # Panics
    ///
    /// Panics if the `sample_rate` is outside the allowed range.
    #[inline]
    pub fn sample_rate(self, sample_rate: f32) -> Self {
        if !(0.0..=1.0).contains(&sample_rate) {
            panic!("Sample rate {sample_rate} is outside the allowed range [0.0, 1.0].")
        }

        Self {
            sample_rate,
            ..self
        }
    }

    /// Sets the [sample rate](field@ClientOptions::traces_sample_rate) for tracing transactions.
    ///
    /// Must be between `0.0` and `1.0`. Defaults to `0.0`.
    ///
    /// # Panics
    ///
    /// Panics if the `traces_sample_rate` is outside the allowed range.
    #[inline]
    pub fn traces_sample_rate(self, traces_sample_rate: f32) -> Self {
        if !(0.0..=1.0).contains(&traces_sample_rate) {
            panic!(
                "Traces sample rate {traces_sample_rate} is outside the allowed range [0.0, 1.0]."
            )
        }

        Self {
            traces_sample_rate,
            ..self
        }
    }

    /// Sets the [sampler callback](field@ClientOptions::traces_sampler) for tracing transactions.
    ///
    /// Return a sample rate between `0.0` and `1.0` for the transaction in question. Takes
    /// priority over [`traces_sample_rate`](method@ClientOptions::traces_sample_rate).
    #[inline]
    pub fn traces_sampler<F>(self, traces_sampler: F) -> Self
    where
        F: Fn(&TransactionContext) -> f32 + Send + Sync + 'static,
    {
        let traces_sampler = Some(Arc::new(traces_sampler) as Arc<TracesSampler>);
        Self {
            traces_sampler,
            ..self
        }
    }

    /// Sets the [maximum number of breadcrumbs](field@ClientOptions::max_breadcrumbs).
    ///
    /// Defaults to `100`.
    #[inline]
    pub fn max_breadcrumbs(self, max_breadcrumbs: usize) -> Self {
        Self {
            max_breadcrumbs,
            ..self
        }
    }

    /// Enables or disables [attaching stacktraces](field@ClientOptions::attach_stacktrace) to
    /// messages.
    ///
    /// Defaults to `false`.
    #[inline]
    pub fn attach_stacktrace(self, attach_stacktrace: bool) -> Self {
        Self {
            attach_stacktrace,
            ..self
        }
    }

    /// Enables or disables sending [default PII](field@ClientOptions::send_default_pii).
    ///
    /// This includes information such as potentially sensitive HTTP headers and user IP addresses
    /// in HTTP server integrations. Defaults to `false`.
    #[inline]
    pub fn send_default_pii(self, send_default_pii: bool) -> Self {
        Self {
            send_default_pii,
            ..self
        }
    }

    /// Sets the [server name](field@ClientOptions::server_name) to be reported.
    #[inline]
    pub fn server_name<T>(self, server_name: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        let server_name = Some(server_name.into());
        Self {
            server_name,
            ..self
        }
    }

    /// Sets [module prefixes](field@ClientOptions::in_app_include) that are always considered
    /// in-app.
    #[inline]
    pub fn in_app_include<I>(self, in_app_include: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        let in_app_include = in_app_include.into_iter().collect();
        Self {
            in_app_include,
            ..self
        }
    }

    /// Sets [module prefixes](field@ClientOptions::in_app_exclude) that are never considered
    /// in-app.
    #[inline]
    pub fn in_app_exclude<I>(self, in_app_exclude: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        let in_app_exclude = in_app_exclude.into_iter().collect();
        Self {
            in_app_exclude,
            ..self
        }
    }

    /// Sets the [integrations](field@ClientOptions::integrations) to enable, replacing the
    /// existing list.
    ///
    /// See [`sentry::integrations`](integrations/index.html#installing-integrations) for how to
    /// use this to enable extra integrations. Use
    /// [`add_integration`](method@ClientOptions::add_integration) to append.
    #[inline]
    pub fn integrations<I>(self, integrations: I) -> Self
    where
        I: IntoIterator<Item = Arc<dyn Integration>>,
    {
        let integrations = integrations.into_iter().collect();
        Self {
            integrations,
            ..self
        }
    }

    /// Enables or disables [default integrations](field@ClientOptions::default_integrations).
    ///
    /// See [`sentry::integrations`](integrations/index.html#default-integrations) for details.
    /// Defaults to `true`.
    #[inline]
    pub fn default_integrations(self, default_integrations: bool) -> Self {
        Self {
            default_integrations,
            ..self
        }
    }

    /// Sets the [callback](field@ClientOptions::before_send) that is executed before event
    /// sending.
    #[inline]
    pub fn before_send<F>(self, before_send: F) -> Self
    where
        F: Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        let before_send = Some(Arc::new(before_send) as BeforeCallback<Event<'static>>);
        Self {
            before_send,
            ..self
        }
    }

    /// Sets the [callback](field@ClientOptions::before_breadcrumb) that is executed before adding
    /// each breadcrumb.
    #[inline]
    pub fn before_breadcrumb<F>(self, before_breadcrumb: F) -> Self
    where
        F: Fn(Breadcrumb) -> Option<Breadcrumb> + Send + Sync + 'static,
    {
        let before_breadcrumb = Some(Arc::new(before_breadcrumb) as BeforeCallback<Breadcrumb>);
        Self {
            before_breadcrumb,
            ..self
        }
    }

    /// Sets the [callback](field@ClientOptions::before_send_log) that is executed before sending
    /// each log.
    #[cfg(feature = "logs")]
    #[inline]
    pub fn before_send_log<F>(self, before_send_log: F) -> Self
    where
        F: Fn(Log) -> Option<Log> + Send + Sync + 'static,
    {
        let before_send_log = Some(Arc::new(before_send_log) as BeforeCallback<Log>);
        Self {
            before_send_log,
            ..self
        }
    }

    /// Sets the [callback](field@ClientOptions::before_send_metric) that is executed before
    /// sending each metric.
    ///
    /// This callback can modify a metric or return `None` to drop it.
    #[cfg(feature = "metrics")]
    #[inline]
    pub fn before_send_metric<F>(self, before_send_metric: F) -> Self
    where
        F: Fn(Metric) -> Option<Metric> + Send + Sync + 'static,
    {
        let before_send_metric = Some(Arc::new(before_send_metric) as BeforeCallback<Metric>);
        Self {
            before_send_metric,
            ..self
        }
    }

    /// Sets the [transport](field@ClientOptions::transport) to use.
    ///
    /// This is typically either a function taking the client options by reference and returning a
    /// transport, an `Arc<Transport>`, or the `DefaultTransportFactory`. Types that do not
    /// implement [`TransportFactory`] use direct field assignment.
    #[inline]
    pub fn transport<T: TransportFactory + 'static>(self, transport: T) -> Self {
        let transport = Some(Arc::new(transport) as Arc<dyn TransportFactory>);
        Self { transport, ..self }
    }

    /// Sets the optional [HTTP proxy](field@ClientOptions::http_proxy) to use.
    ///
    /// This defaults to the `http_proxy` environment variable.
    #[inline]
    pub fn http_proxy<T>(self, http_proxy: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        let http_proxy = Some(http_proxy.into());
        Self { http_proxy, ..self }
    }

    /// Sets the optional [HTTPS proxy](field@ClientOptions::https_proxy) to use.
    ///
    /// This defaults to the `HTTPS_PROXY` environment variable, or `http_proxy` if that one
    /// exists.
    #[inline]
    pub fn https_proxy<T>(self, https_proxy: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        let https_proxy = Some(https_proxy.into());
        Self {
            https_proxy,
            ..self
        }
    }

    /// Sets the [shutdown drain timeout](field@ClientOptions::shutdown_timeout).
    ///
    /// Defaults to 2 seconds.
    #[inline]
    pub fn shutdown_timeout(self, shutdown_timeout: Duration) -> Self {
        Self {
            shutdown_timeout,
            ..self
        }
    }

    /// Sets the [maximum request body size](field@ClientOptions::max_request_body_size) to
    /// capture.
    ///
    /// Controls the maximum size of an HTTP request body that can be captured when using HTTP
    /// server integrations. Needs [`send_default_pii`](method@ClientOptions::send_default_pii) to
    /// be enabled to have any effect. Defaults to [`MaxRequestBodySize::Medium`].
    #[inline]
    pub fn max_request_body_size(self, max_request_body_size: MaxRequestBodySize) -> Self {
        Self {
            max_request_body_size,
            ..self
        }
    }

    /// Enables or disables sending [structured logs](field@ClientOptions::enable_logs).
    ///
    /// The `logs` feature is required to capture logs. Defaults to `true`.
    #[inline]
    pub fn enable_logs(self, enable_logs: bool) -> Self {
        Self {
            enable_logs,
            ..self
        }
    }

    /// Enables or disables [metric capture APIs](field@ClientOptions::enable_metrics).
    ///
    /// The `metrics` feature is required to capture metrics. Defaults to `true`.
    #[inline]
    pub fn enable_metrics(self, enable_metrics: bool) -> Self {
        Self {
            enable_metrics,
            ..self
        }
    }

    /// Enables or disables
    /// [accepting invalid TLS certificates](field@ClientOptions::accept_invalid_certs).
    ///
    /// This introduces significant vulnerabilities, and should only be used as a last resort.
    /// Defaults to `false`.
    #[inline]
    pub fn accept_invalid_certs(self, accept_invalid_certs: bool) -> Self {
        Self {
            accept_invalid_certs,
            ..self
        }
    }

    /// Enables or disables
    /// [automatic session tracking](field@ClientOptions::auto_session_tracking).
    ///
    /// When enabled, a new "user-mode" session is started at `sentry::init` and persists for the
    /// application lifetime. Defaults to `false`.
    #[cfg(feature = "release-health")]
    #[inline]
    pub fn auto_session_tracking(self, auto_session_tracking: bool) -> Self {
        Self {
            auto_session_tracking,
            ..self
        }
    }

    /// Sets how [sessions are tracked](field@ClientOptions::session_mode).
    ///
    /// See [`SessionMode`] for the available modes. Defaults to [`SessionMode::Application`].
    #[cfg(feature = "release-health")]
    #[inline]
    pub fn session_mode(self, session_mode: SessionMode) -> Self {
        Self {
            session_mode,
            ..self
        }
    }

    /// Sets the [user agent](field@ClientOptions::user_agent) that should be reported.
    ///
    /// Defaults to the SDK user agent.
    #[inline]
    pub fn user_agent<T>(self, user_agent: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        let user_agent = user_agent.into();
        Self { user_agent, ..self }
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
    #[inline]
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
        let before_send_log = {
            #[derive(Debug)]
            struct BeforeSendLog;
            self.before_send_log.as_ref().map(|_| BeforeSendLog)
        };
        let before_send_metric = {
            #[derive(Debug)]
            struct BeforeSendMetric;
            self.before_send_metric.as_ref().map(|_| BeforeSendMetric)
        };
        #[derive(Debug)]
        struct TransportFactory;

        let integrations: Vec<_> = self.integrations.iter().map(|i| i.name()).collect();

        let mut debug_struct = f.debug_struct("ClientOptions");
        debug_struct
            .field("dsn", &self.dsn)
            .field("debug", &self.debug)
            .field("release", &self.release)
            .field("environment", &self.environment)
            .field("sample_rate", &self.sample_rate)
            .field("traces_sample_rate", &self.traces_sample_rate)
            .field(
                "traces_sampler",
                &self
                    .traces_sampler
                    .as_ref()
                    .map(|arc| std::ptr::addr_of!(**arc)),
            )
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
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .field("auto_session_tracking", &self.auto_session_tracking)
            .field("session_mode", &self.session_mode)
            .field("enable_logs", &self.enable_logs)
            .field("before_send_log", &before_send_log)
            .field("enable_metrics", &self.enable_metrics)
            .field("before_send_metric", &before_send_metric)
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
            traces_sample_rate: 0.0,
            traces_sampler: None,
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
            accept_invalid_certs: false,
            auto_session_tracking: false,
            session_mode: SessionMode::Application,
            user_agent: Cow::Borrowed(USER_AGENT),
            max_request_body_size: MaxRequestBodySize::Medium,
            enable_logs: true,
            before_send_log: None,
            enable_metrics: true,
            before_send_metric: None,
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
