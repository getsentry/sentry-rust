use std::env;
use std::{borrow::Cow, sync::Arc};

use crate::transports::DefaultTransportFactory;

use crate::internals::Dsn;
use crate::{ClientOptions, Integration};

/// Apply default client options.
///
/// Extends the given `ClientOptions` with default options such as a default
/// transport, a set of default integrations if not requested otherwise, and
/// also sets the `dsn`, `release`, `environment`, and proxy settings based on
/// environment variables.
///
/// # Examples
/// ```
/// std::env::set_var("SENTRY_RELEASE", "release-from-env");
///
/// let options = sentry::ClientOptions::default();
/// assert_eq!(options.release, None);
/// assert!(options.transport.is_none());
///
/// let options = sentry::internals::apply_defaults(options);
/// assert_eq!(options.release, Some("release-from-env".into()));
/// assert!(options.transport.is_some());
/// ```
pub fn apply_defaults(mut opts: ClientOptions) -> ClientOptions {
    if opts.transport.is_none() {
        opts.transport = Some(Arc::new(DefaultTransportFactory));
    }
    if opts.default_integrations {
        // default integrations need to be ordered *before* custom integrations,
        // since they also process events in order
        let mut integrations: Vec<Arc<dyn Integration>> = vec![];
        #[cfg(feature = "with_backtrace")]
        {
            integrations.push(Arc::new(
                sentry_backtrace::AttachStacktraceIntegration::default(),
            ));
        }
        integrations.push(Arc::new(sentry_contexts::ContextIntegration::default()));
        #[cfg(feature = "with_failure")]
        {
            integrations.push(Arc::new(sentry_failure::FailureIntegration::default()));
        }
        #[cfg(feature = "with_panic")]
        {
            #[allow(unused_mut)]
            let mut integration = sentry_panic::PanicIntegration::default();
            #[cfg(feature = "with_failure")]
            {
                integration = integration.add_extractor(sentry_failure::panic_extractor);
            }
            integrations.push(Arc::new(integration));
        }
        #[cfg(feature = "with_backtrace")]
        {
            integrations.push(Arc::new(
                sentry_backtrace::ProcessStacktraceIntegration::default(),
            ));
        }
        integrations.extend(opts.integrations.into_iter());
        opts.integrations = integrations;
    }
    if opts.dsn.is_none() {
        opts.dsn = env::var("SENTRY_DSN")
            .ok()
            .and_then(|dsn| dsn.parse::<Dsn>().ok());
    }
    if opts.release.is_none() {
        opts.release = env::var("SENTRY_RELEASE").ok().map(Cow::Owned);
    }
    if opts.environment.is_none() {
        opts.environment = env::var("SENTRY_ENVIRONMENT")
            .ok()
            .map(Cow::Owned)
            .or_else(|| {
                Some(Cow::Borrowed(if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }))
            });
    }
    if opts.http_proxy.is_none() {
        opts.http_proxy = std::env::var("HTTP_PROXY")
            .ok()
            .map(Cow::Owned)
            .or_else(|| std::env::var("http_proxy").ok().map(Cow::Owned));
    }
    if opts.https_proxy.is_none() {
        opts.https_proxy = std::env::var("HTTPS_PROXY")
            .ok()
            .map(Cow::Owned)
            .or_else(|| std::env::var("https_proxy").ok().map(Cow::Owned))
            .or_else(|| opts.http_proxy.clone());
    }
    opts
}
