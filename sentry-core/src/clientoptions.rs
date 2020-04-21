use std::borrow::Cow;
use std::fmt;
use std::panic::RefUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use crate::constants::USER_AGENT;
use crate::internals::Dsn;
use crate::protocol::{Breadcrumb, Event};
use crate::transport::{DefaultTransportFactory, TransportFactory};
use crate::utils;

/// Type alias for before event/breadcrumb handlers.
pub type BeforeCallback<T> = Arc<Box<dyn Fn(T) -> Option<T> + Send + Sync>>;

/// Configuration settings for the client.
pub struct ClientOptions {
    /// The DSN to use.  If not set the client is effectively disabled.
    pub dsn: Option<Dsn>,
    /// The transport to use.
    ///
    /// This is typically either a boxed function taking the client options by
    /// reference and returning a `Transport`, a boxed `Arc<Transport>` or
    /// alternatively the `DefaultTransportFactory`.
    pub transport: Box<dyn TransportFactory>,
    /// module prefixes that are always considered in_app
    pub in_app_include: Vec<&'static str>,
    /// module prefixes that are never in_app
    pub in_app_exclude: Vec<&'static str>,
    /// border frames which indicate a border from a backtrace to
    /// useless internals.  Some are automatically included.
    pub extra_border_frames: Vec<&'static str>,
    /// Maximum number of breadcrumbs (0 to disable feature).
    pub max_breadcrumbs: usize,
    /// Automatically trim backtraces of junk before sending.
    pub trim_backtraces: bool,
    /// The release to be sent with events.
    pub release: Option<Cow<'static, str>>,
    /// The environment to be sent with events.
    pub environment: Option<Cow<'static, str>>,
    /// The server name to be reported.
    pub server_name: Option<Cow<'static, str>>,
    /// The sample rate for event submission (0.0 - 1.0, defaults to 1.0)
    pub sample_rate: f32,
    /// The user agent that should be reported.
    pub user_agent: Cow<'static, str>,
    /// An optional HTTP proxy to use.
    ///
    /// This will default to the `http_proxy` environment variable.
    pub http_proxy: Option<Cow<'static, str>>,
    /// An optional HTTPS proxy to use.
    ///
    /// This will default to the `HTTPS_PROXY` environment variable
    /// or `http_proxy` if that one exists.
    pub https_proxy: Option<Cow<'static, str>>,
    /// The timeout on client drop for draining events on shutdown.
    pub shutdown_timeout: Duration,
    /// Enables debug mode.
    ///
    /// In debug mode debug information is printed to stderr to help you understand what
    /// sentry is doing.  When the `with_debug_to_log` flag is enabled Sentry will instead
    /// log to the `sentry` logger independently of this flag with the `Debug` level.
    pub debug: bool,
    /// Attaches stacktraces to messages.
    pub attach_stacktrace: bool,
    /// If turned on some default PII informat is attached.
    pub send_default_pii: bool,
    /// Before send callback.
    pub before_send: Option<BeforeCallback<Event<'static>>>,
    /// Before breadcrumb add callback.
    pub before_breadcrumb: Option<BeforeCallback<Breadcrumb>>,
}

// make this unwind safe.  It's not out of the box because of the contained `BeforeCallback`s.
impl RefUnwindSafe for ClientOptions {}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct TransportFactory;
        #[derive(Debug)]
        struct BeforeSendSet(bool);
        #[derive(Debug)]
        struct BeforeBreadcrumbSet(bool);
        f.debug_struct("ClientOptions")
            .field("dsn", &self.dsn)
            .field("transport", &TransportFactory)
            .field("in_app_include", &self.in_app_include)
            .field("in_app_exclude", &self.in_app_exclude)
            .field("extra_border_frames", &self.extra_border_frames)
            .field("max_breadcrumbs", &self.max_breadcrumbs)
            .field("trim_backtraces", &self.trim_backtraces)
            .field("release", &self.release)
            .field("environment", &self.environment)
            .field("server_name", &self.server_name)
            .field("sample_rate", &self.sample_rate)
            .field("user_agent", &self.user_agent)
            .field("http_proxy", &self.http_proxy)
            .field("https_proxy", &self.https_proxy)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("debug", &self.debug)
            .field("attach_stacktrace", &self.attach_stacktrace)
            .field("send_default_pii", &self.send_default_pii)
            .field("before_send", &BeforeSendSet(self.before_send.is_some()))
            .field(
                "before_breadcrumb",
                &BeforeBreadcrumbSet(self.before_breadcrumb.is_some()),
            )
            .finish()
    }
}

impl Clone for ClientOptions {
    fn clone(&self) -> ClientOptions {
        ClientOptions {
            dsn: self.dsn.clone(),
            transport: self.transport.clone_factory(),
            in_app_include: self.in_app_include.clone(),
            in_app_exclude: self.in_app_exclude.clone(),
            extra_border_frames: self.extra_border_frames.clone(),
            max_breadcrumbs: self.max_breadcrumbs,
            trim_backtraces: self.trim_backtraces,
            release: self.release.clone(),
            environment: self.environment.clone(),
            server_name: self.server_name.clone(),
            sample_rate: self.sample_rate,
            user_agent: self.user_agent.clone(),
            http_proxy: self.http_proxy.clone(),
            https_proxy: self.https_proxy.clone(),
            shutdown_timeout: self.shutdown_timeout,
            debug: self.debug,
            attach_stacktrace: self.attach_stacktrace,
            send_default_pii: self.send_default_pii,
            before_send: self.before_send.clone(),
            before_breadcrumb: self.before_breadcrumb.clone(),
        }
    }
}

impl Default for ClientOptions {
    fn default() -> ClientOptions {
        ClientOptions {
            // any invalid dsn including the empty string disables the dsn
            dsn: std::env::var("SENTRY_DSN")
                .ok()
                .and_then(|dsn| dsn.parse::<Dsn>().ok()),
            transport: Box::new(DefaultTransportFactory),
            in_app_include: vec![],
            in_app_exclude: vec![],
            extra_border_frames: vec![],
            max_breadcrumbs: 100,
            trim_backtraces: true,
            release: std::env::var("SENTRY_RELEASE").ok().map(Cow::Owned),
            environment: std::env::var("SENTRY_ENVIRONMENT")
                .ok()
                .map(Cow::Owned)
                .or_else(|| {
                    Some(Cow::Borrowed(if cfg!(debug_assertions) {
                        "debug"
                    } else {
                        "release"
                    }))
                }),
            server_name: utils::server_name().map(Cow::Owned),
            sample_rate: 1.0,
            user_agent: Cow::Borrowed(&USER_AGENT),
            http_proxy: std::env::var("http_proxy").ok().map(Cow::Owned),
            https_proxy: std::env::var("https_proxy")
                .ok()
                .map(Cow::Owned)
                .or_else(|| std::env::var("HTTPS_PROXY").ok().map(Cow::Owned))
                .or_else(|| std::env::var("http_proxy").ok().map(Cow::Owned)),
            shutdown_timeout: Duration::from_secs(2),
            debug: false,
            attach_stacktrace: false,
            send_default_pii: false,
            before_send: None,
            before_breadcrumb: None,
        }
    }
}
