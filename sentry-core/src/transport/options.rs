//! Includes the [`TransportOptions`] struct.

use std::borrow::Cow;

use sentry_types::Dsn;

use crate::ClientOptions;

/// Options for a transport.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct TransportOptions {
    /// The transport's Sentry DSN.
    pub dsn: Dsn,
    /// The user agent sent with transport requests.
    pub user_agent: Cow<'static, str>,
    /// An optional HTTP proxy to use.
    pub http_proxy: Option<Cow<'static, str>>,
    /// An optional HTTPS proxy to use.
    pub https_proxy: Option<Cow<'static, str>>,
    /// Whether TLS certificate validation should be disabled.
    pub accept_invalid_certs: bool,
}

impl TransportOptions {
    /// Try to convert a [`&ClientOptions`](ClientOptions) to a [`TransportOptions`] by extracting
    /// the relevant fields from the `ClientOptions`.
    ///
    /// This method is provided so that code which expects [`TransportOptions`] can be
    /// backwards-compatible with older code, which provides `ClientOptions`.
    ///
    /// Returns [`None`] if `options.dsn` is `None`, `Some(_)` otherwise.
    pub fn try_from_client_options(options: &ClientOptions) -> Option<Self> {
        let ClientOptions {
            dsn,
            http_proxy,
            https_proxy,
            accept_invalid_certs,
            user_agent,
            ..
        } = options;

        dsn.as_ref().cloned().map(|dsn| Self {
            dsn,
            user_agent: user_agent.clone(),
            http_proxy: http_proxy.clone(),
            https_proxy: https_proxy.clone(),
            accept_invalid_certs: *accept_invalid_certs,
        })
    }

    /// Converts these [`TransportOptions`] into [`ClientOptions`].
    ///
    /// This method is provided for backwards-compatibility with custom transports which cannot
    /// be contructed from [`TransportOptions`] because they expect [`ClientOptions`].
    ///
    /// Any fields on [`ClientOptions`] which are not present in [`TransportOptions`] will be
    /// set to their default values.
    pub(crate) fn into_client_options(self) -> ClientOptions {
        let Self {
            dsn,
            user_agent,
            http_proxy,
            https_proxy,
            accept_invalid_certs,
        } = self;

        let dsn = Some(dsn);

        ClientOptions {
            dsn,
            user_agent,
            http_proxy,
            https_proxy,
            accept_invalid_certs,
            ..Default::default()
        }
    }
}
