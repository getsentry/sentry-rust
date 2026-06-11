use std::time::Duration;

use reqwest::{header as ReqwestHeaders, Client as ReqwestClient, Proxy, StatusCode};
use sentry_core::TransportOptions;

use super::{
    tokio_thread::{TransportThread, TransportThreadOptions},
    RateLimiter, HTTP_PAYLOAD_TOO_LARGE, HTTP_PAYLOAD_TOO_LARGE_MESSAGE,
};

use crate::{sentry_debug, ClientOptions, Envelope, Transport};

/// A [`Transport`] that sends events via the [`reqwest`] library.
///
/// When the `transport` feature is enabled this will currently
/// be the default transport.  This is separately enabled by the
/// `reqwest` feature flag.
#[cfg_attr(doc_cfg, doc(cfg(feature = "reqwest")))]
pub struct ReqwestHttpTransport {
    thread: TransportThread,
}

/// Options for constructing a [`ReqwestHttpTransport`].
///
/// Currently, this is primarily a wrapper around a [`TransportOptions`], and must be created with
/// the `From<TransportOptions>` implementation. Optionally, a [`reqwest::Client`] for the
/// transport may be provided with [`Self::with_client`].
#[derive(Debug)]
#[must_use]
pub struct ReqwestHttpTransportOptions {
    general_options: TransportOptions,
    client: Option<ReqwestClient>,
}

impl ReqwestHttpTransport {
    /// Backwards-compatible method for creating a [`ReqwestHttpTransport`].
    ///
    /// Please use [`ReqwestHttpTransportOptions::build`] instead.
    ///
    /// ### Panics
    ///
    /// Panics if called with `options` that lack a DSN.
    #[inline]
    #[deprecated = "use `ReqwestHttpTransportOptions::build` instead"]
    pub fn new(options: &ClientOptions) -> Self {
        let general_options = TransportOptions::try_from_client_options(options)
            .expect("this method should only be called when options has a DSN");

        ReqwestHttpTransportOptions::from(general_options).build()
    }

    /// Backwards-compatible method for creating a [`ReqwestHttpTransport`] that uses the specified
    /// [`ReqwestClient`].
    ///
    /// Please use [`ReqwestHttpTransportOptions::build`] instead.
    ///
    /// ### Panics
    ///
    /// Panics if called with `options` that lack a DSN.
    #[inline]
    #[deprecated = "use `ReqwestHttpTransportOptions::build` instead"]
    pub fn with_client(options: &ClientOptions, client: ReqwestClient) -> Self {
        let general_options = TransportOptions::try_from_client_options(options)
            .expect("this method should only be called when options has a DSN");

        ReqwestHttpTransportOptions::from(general_options)
            .with_client(client)
            .build()
    }

    /// Creates a new [`ReqwestHttpTransport`] with the given `options`.
    #[inline]
    pub(super) fn with_options(options: ReqwestHttpTransportOptions) -> Self {
        let ReqwestHttpTransportOptions {
            general_options:
                TransportOptions {
                    dsn,
                    user_agent,
                    http_proxy,
                    https_proxy,
                    accept_invalid_certs,
                    ..
                },
            client,
        } = options;

        let client = client.unwrap_or_else(|| {
            let mut builder = reqwest::Client::builder();
            if accept_invalid_certs {
                builder = builder.danger_accept_invalid_certs(true);
            }
            if let Some(url) = http_proxy.as_ref() {
                match Proxy::http(url.as_ref()) {
                    Ok(proxy) => {
                        builder = builder.proxy(proxy);
                    }
                    Err(err) => {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                }
            };
            if let Some(url) = https_proxy.as_ref() {
                match Proxy::https(url.as_ref()) {
                    Ok(proxy) => {
                        builder = builder.proxy(proxy);
                    }
                    Err(err) => {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                }
            };
            builder
                .build()
                .expect("Failed to build `reqwest` client as a TLS backend is not available. Enable either the `native-tls` or the `rustls` feature of the `sentry` crate.")
        });

        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();

        let send_fn = move |envelope: Envelope, mut rl: RateLimiter| {
            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            let request = client.post(&url).header("X-Sentry-Auth", &auth).body(body);

            // NOTE: because of lifetime issues, building the request using the
            // `client` has to happen outside of this async block.
            async move {
                match request.send().await {
                    Ok(response) => {
                        let headers = response.headers();

                        if let Some(sentry_header) = headers
                            .get("x-sentry-rate-limits")
                            .and_then(|x| x.to_str().ok())
                        {
                            rl.update_from_sentry_header(sentry_header);
                        } else if let Some(retry_after) = headers
                            .get(ReqwestHeaders::RETRY_AFTER)
                            .and_then(|x| x.to_str().ok())
                        {
                            rl.update_from_retry_after(retry_after);
                        } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
                            rl.update_from_429();
                        }

                        let is_payload_too_large =
                            response.status().as_u16() == HTTP_PAYLOAD_TOO_LARGE;
                        match response.text().await {
                            Err(err) => {
                                sentry_debug!("Failed to read sentry response: {}", err);
                            }
                            Ok(text) => {
                                sentry_debug!("Get response: `{}`", text);
                            }
                        }
                        if is_payload_too_large {
                            sentry_debug!("{HTTP_PAYLOAD_TOO_LARGE_MESSAGE}");
                        }
                    }
                    Err(err) => {
                        sentry_debug!("Failed to send envelope: {}", err);
                    }
                }
                rl
            }
        };

        let thread = TransportThreadOptions::new(send_fn).spawn_thread();
        Self { thread }
    }
}

impl Transport for ReqwestHttpTransport {
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

impl From<TransportOptions> for ReqwestHttpTransportOptions {
    #[inline]
    fn from(value: TransportOptions) -> Self {
        Self {
            general_options: value,
            client: None,
        }
    }
}

impl ReqwestHttpTransportOptions {
    /// Specify the [`reqwest::Client`] for the [`ReqwestHttpTransport`].
    #[inline]
    pub fn with_client(self, client: ReqwestClient) -> Self {
        let client = Some(client);
        Self { client, ..self }
    }

    /// Create a [`ReqwestHttpTransport`] using these options.
    #[inline]
    pub fn build(self) -> ReqwestHttpTransport {
        ReqwestHttpTransport::with_options(self)
    }
}
