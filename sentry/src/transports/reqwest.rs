use std::time::Duration;

use reqwest_::{header as ReqwestHeaders, Client as ReqwestClient, Proxy};

use super::thread::TransportThread;

use crate::{sentry_debug, ClientOptions, Envelope, Transport};

/// A [`Transport`] that sends events via the [`reqwest`] library.
///
/// When the `transport` feature is enabled this will currently
/// be the default transport.  This is separately enabled by the
/// `reqwest` feature flag.
///
/// [`reqwest`]: https://crates.io/crates/reqwest
#[cfg_attr(doc_cfg, doc(cfg(feature = "reqwest")))]
pub struct ReqwestHttpTransport {
    thread: TransportThread,
}

#[cfg_attr(doc_cfg, doc(cfg(feature = "reqwest")))]
impl ReqwestHttpTransport {
    /// Creates a new Transport.
    pub fn new(options: &ClientOptions) -> Self {
        Self::new_internal(options, None)
    }

    /// Creates a new Transport that uses the specified [`ReqwestClient`].
    pub fn with_client(options: &ClientOptions, client: ReqwestClient) -> Self {
        Self::new_internal(options, Some(client))
    }

    fn new_internal(options: &ClientOptions, client: Option<ReqwestClient>) -> Self {
        let client = client.unwrap_or_else(|| {
            let mut builder = reqwest_::Client::builder();
            if let Some(url) = options.http_proxy.as_ref() {
                builder = builder.proxy(Proxy::http(url.as_ref()).unwrap());
            };
            if let Some(url) = options.https_proxy.as_ref() {
                builder = builder.proxy(Proxy::https(url.as_ref()).unwrap());
            };
            builder.build().unwrap()
        });
        let dsn = options.dsn.as_ref().unwrap();
        let user_agent = options.user_agent.to_owned();
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();

        let thread = TransportThread::new(move |envelope, mut rl| {
            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            let request = client.post(&url).header("X-Sentry-Auth", &auth).body(body);

            // NOTE: because of lifetime issues, building the request using the
            // `client` has to happen outside of this async block.
            async move {
                match request.send().await {
                    Ok(response) => {
                        let headers = response.headers();
                        if let Some(retry_after) = headers
                            .get(ReqwestHeaders::RETRY_AFTER)
                            .and_then(|x| x.to_str().ok())
                        {
                            rl.update_from_retry_after(retry_after);
                        }
                        if let Some(sentry_header) = headers
                            .get("x-sentry-rate-limits")
                            .and_then(|x| x.to_str().ok())
                        {
                            rl.update_from_sentry_header(sentry_header);
                        }
                        match response.text().await {
                            Err(err) => {
                                sentry_debug!("Failed to read sentry response: {}", err);
                            }
                            Ok(text) => {
                                sentry_debug!("Get response: `{}`", text);
                            }
                        }
                    }
                    Err(err) => {
                        sentry_debug!("Failed to send envelope: {}", err);
                    }
                }
                rl
            }
        });
        Self { thread }
    }
}

#[cfg_attr(doc_cfg, doc(cfg(feature = "reqwest")))]
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
