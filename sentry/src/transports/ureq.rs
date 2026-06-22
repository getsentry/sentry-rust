use std::time::Duration;

use sentry_core::client_report::Reason as LossReason;
use sentry_core::TransportOptions;
use ureq::http::Response;
#[cfg(any(
    feature = "rustls",
    feature = "rustls-no-provider",
    feature = "native-tls"
))]
use ureq::tls::{TlsConfig, TlsProvider};
use ureq::{Agent, Proxy};

use super::{
    thread::{TransportThread, TransportThreadOptions},
    RateLimiter, HTTP_PAYLOAD_TOO_LARGE, HTTP_PAYLOAD_TOO_LARGE_MESSAGE,
};

use crate::{sentry_debug, types::Scheme, ClientOptions, Envelope, Transport};

/// The status code returned for rate-limited envelopes.
const HTTP_RATE_LIMIT_STATUS: u16 = 429;

/// A [`Transport`] that sends events via the [`ureq`] library.
///
/// This is enabled by the `ureq` feature flag.
#[cfg_attr(doc_cfg, doc(cfg(feature = "ureq")))]
pub struct UreqHttpTransport {
    thread: TransportThread,
}

/// Options for constructing a [`UreqHttpTransport`].
///
/// Currently, this is primarily a wrapper around a [`TransportOptions`], and must be created with
/// the `From<TransportOptions>` implementation. Optionally, a [`ureq::Agent`] for the transport may
/// be provided with [`Self::with_agent`].
#[derive(Debug)]
#[must_use]
pub struct UreqHttpTransportOptions {
    general_options: TransportOptions,
    agent: Option<Agent>,
}

impl UreqHttpTransport {
    /// Backwards-compatible method for creating a [`UreqHttpTransport`].
    ///
    /// Please use [`UreqHttpTransportOptions::build`] instead.
    ///
    /// ### Panics
    ///
    /// Panics if called with `options` that lack a DSN.
    #[inline]
    #[deprecated = "use `UreqHttpTransportOptions::build` instead"]
    pub fn new(options: &ClientOptions) -> Self {
        let general_options = TransportOptions::try_from_client_options(options)
            .expect("this method should only be called when options has a DSN");

        UreqHttpTransportOptions::from(general_options).build()
    }

    /// Backwards-compatible method for creating a [`UreqHttpTransport`] that uses the specified
    /// [`ureq::Agent`].
    ///
    /// Please use [`UreqHttpTransportOptions::build`] instead.
    ///
    /// ### Panics
    ///
    /// Panics if called with `options` that lack a DSN.
    #[inline]
    #[deprecated = "use `UreqHttpTransportOptions::build` instead"]
    pub fn with_agent(options: &ClientOptions, agent: Agent) -> Self {
        let general_options = TransportOptions::try_from_client_options(options)
            .expect("this method should only be called when options has a DSN");

        UreqHttpTransportOptions::from(general_options)
            .with_agent(agent)
            .build()
    }

    /// Creates a new [`UreqHttpTransport`] with the given `options`.
    #[inline]
    pub(super) fn with_options(options: UreqHttpTransportOptions) -> Self {
        let UreqHttpTransportOptions {
            general_options:
                TransportOptions {
                    dsn,
                    user_agent,
                    http_proxy,
                    https_proxy,
                    #[cfg(any(
                        feature = "native-tls",
                        feature = "rustls",
                        feature = "rustls-no-provider"
                    ))]
                    accept_invalid_certs,
                    client_report_recorder,
                    ..
                },
            agent,
        } = options;
        let scheme = dsn.scheme();
        let agent = agent.unwrap_or_else(|| {
            let mut builder = Agent::config_builder();

            #[cfg(feature = "native-tls")]
            {
                builder = builder.tls_config(
                    TlsConfig::builder()
                        .provider(TlsProvider::NativeTls)
                        .disable_verification(accept_invalid_certs)
                        .build(),
                );
            }
            #[cfg(any(feature = "rustls", feature = "rustls-no-provider"))]
            {
                builder = builder.tls_config(
                    TlsConfig::builder()
                        .provider(TlsProvider::Rustls)
                        .disable_verification(accept_invalid_certs)
                        .build(),
                );
            }

            let mut maybe_proxy = None;

            match (scheme, &http_proxy, &https_proxy) {
                (Scheme::Https, _, Some(proxy)) => match Proxy::new(proxy.as_ref()) {
                    Ok(proxy) => {
                        maybe_proxy = Some(proxy);
                    }
                    Err(err) => {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                },
                (_, Some(proxy), _) => match Proxy::new(proxy.as_ref()) {
                    Ok(proxy) => {
                        maybe_proxy = Some(proxy);
                    }
                    Err(err) => {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                },
                _ => {}
            }

            builder = builder.proxy(maybe_proxy);

            builder.build().new_agent()
        });
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();

        let send_fn = move |envelope: Envelope, rl: &mut RateLimiter| {
            let mut body = Vec::new();
            envelope
                .to_writer(&mut body)
                .inspect_err(|_| {
                    client_report_recorder.record_lost_data(&envelope, LossReason::InternalError);
                })
                .expect("envelope should serialize successfully");
            let request = agent
                .post(&url)
                .header("X-Sentry-Auth", &auth)
                .config()
                .http_status_as_error(false)
                .build()
                .send(&body);

            match request {
                Ok(mut response) => {
                    fn header_str<'a, B>(response: &'a Response<B>, key: &str) -> Option<&'a str> {
                        response.headers().get(key)?.to_str().ok()
                    }

                    if let Some(sentry_header) = header_str(&response, "x-sentry-rate-limits") {
                        rl.update_from_sentry_header(sentry_header);
                    } else if let Some(retry_after) = header_str(&response, "retry-after") {
                        rl.update_from_retry_after(retry_after);
                    } else if response.status() == HTTP_RATE_LIMIT_STATUS {
                        rl.update_from_429();
                    }

                    let response_status = response.status().as_u16();

                    match response.body_mut().read_to_string() {
                        Err(err) => {
                            sentry_debug!("Failed to read sentry response: {}", err);
                        }
                        Ok(text) => {
                            sentry_debug!("Get response: `{}`", text);
                        }
                    }
                    if response_status == HTTP_PAYLOAD_TOO_LARGE {
                        sentry_debug!("{HTTP_PAYLOAD_TOO_LARGE_MESSAGE}");
                    }

                    if (400..=599).contains(&response_status)
                        && response_status != HTTP_RATE_LIMIT_STATUS
                    {
                        client_report_recorder.record_lost_data(&envelope, LossReason::SendError);
                    }
                }
                Err(err) => {
                    sentry_debug!("Failed to send envelope: {}", err);
                    client_report_recorder.record_lost_data(&envelope, LossReason::NetworkError);
                }
            }
        };

        let thread = TransportThreadOptions::new(send_fn).spawn_thread();
        Self { thread }
    }
}

impl Transport for UreqHttpTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.thread.send(envelope)
    }
    fn flush(&self, timeout: Duration) -> bool {
        self.thread.flush(timeout)
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.flush(timeout)
    }
}

impl From<TransportOptions> for UreqHttpTransportOptions {
    #[inline]
    fn from(value: TransportOptions) -> Self {
        Self {
            general_options: value,
            agent: None,
        }
    }
}

impl UreqHttpTransportOptions {
    /// Specify the [`ureq::Agent`] for the [`UreqHttpTransport`].
    #[inline]
    pub fn with_agent(self, agent: Agent) -> Self {
        let agent = Some(agent);
        Self { agent, ..self }
    }

    /// Create a [`UreqHttpTransport`] using these options.
    #[inline]
    pub fn build(self) -> UreqHttpTransport {
        UreqHttpTransport::with_options(self)
    }
}
