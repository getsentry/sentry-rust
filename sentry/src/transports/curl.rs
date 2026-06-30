use std::io::{Cursor, Read};
use std::time::Duration;

use curl::easy::Easy as CurlClient;
use sentry_core::client_report::Reason as LossReason;
use sentry_core::TransportOptions;

use super::{
    thread::{TransportThread, TransportThreadOptions},
    RateLimiter, HTTP_PAYLOAD_TOO_LARGE, HTTP_PAYLOAD_TOO_LARGE_MESSAGE,
};

use crate::{sentry_debug, types::Scheme, ClientOptions, Envelope, Transport};

/// The status code returned for rate-limited envelopes.
const HTTP_RATE_LIMIT_STATUS: u32 = 429;

/// Parse a single raw header line as delivered by libcurl's `header_function`.
///
/// Returns the lowercased header name and the trimmed value, or `None` if the
/// line is empty, contains a NUL byte, is not valid UTF-8, or has no `:`.
fn parse_curl_header_line(data: &[u8]) -> Option<(String, String)> {
    let line = std::str::from_utf8(data)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let (key, value) = line.split_once(':')?;
    Some((key.trim().to_lowercase(), value.trim().to_string()))
}

/// A [`Transport`] that sends events via the [`curl`] library.
///
/// This is enabled by the `curl` feature flag.
#[cfg_attr(doc_cfg, doc(cfg(feature = "curl")))]
pub struct CurlHttpTransport {
    thread: TransportThread,
}

/// Options for constructing a [`CurlHttpTransport`].
///
/// Currently, this is primarily a wrapper around a [`TransportOptions`], and must be created with
/// the `From<TransportOptions>` implementation. Optionally, a [`curl::easy::Easy`] client for the
/// transport may be provided with [`Self::with_client`].
#[derive(Debug)]
#[must_use]
pub struct CurlHttpTransportOptions {
    general_options: TransportOptions,
    client: Option<CurlClient>,
}

impl CurlHttpTransport {
    /// Backwards-compatible method for creating a [`CurlHttpTransport`].
    ///
    /// Please use [`CurlHttpTransportOptions::build`] instead.
    ///
    /// ### Panics
    ///
    /// Panics if called with `options` that lack a DSN.
    #[inline]
    #[deprecated = "use `CurlHttpTransportOptions::build` instead"]
    pub fn new(options: &ClientOptions) -> Self {
        let general_options = TransportOptions::try_from_client_options(options)
            .expect("this method should only be called when options has a DSN");

        CurlHttpTransportOptions::from(general_options).build()
    }

    /// Backwards-compatible method for creating a [`CurlHttpTransport`] that uses the specified
    /// [`CurlClient`].
    ///
    /// Please use [`CurlHttpTransportOptions::build`] instead.
    ///
    /// ### Panics
    ///
    /// Panics if called with `options` that lack a DSN.
    #[inline]
    #[deprecated = "use `CurlHttpTransportOptions::build` instead"]
    pub fn with_client(options: &ClientOptions, client: CurlClient) -> Self {
        let general_options = TransportOptions::try_from_client_options(options)
            .expect("this method should only be called when options has a DSN");

        CurlHttpTransportOptions::from(general_options)
            .with_client(client)
            .build()
    }

    /// Creates a new [`CurlHttpTransport`] with the given `options`.
    #[inline]
    pub(super) fn with_options(options: CurlHttpTransportOptions) -> Self {
        let CurlHttpTransportOptions {
            general_options:
                TransportOptions {
                    dsn,
                    user_agent,
                    http_proxy,
                    https_proxy,
                    accept_invalid_certs,
                    client_report_recorder,
                    ..
                },
            client,
        } = options;

        let client = client.unwrap_or_else(CurlClient::new);
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();
        let scheme = dsn.scheme();

        let mut handle = client;

        let send_fn_client_report_recorder = client_report_recorder.clone();

        let send_fn = move |envelope: Envelope, rl: &mut RateLimiter| {
            handle.reset();
            handle.url(&url).unwrap();
            handle.custom_request("POST").unwrap();

            if accept_invalid_certs {
                handle.ssl_verify_host(false).unwrap();
                handle.ssl_verify_peer(false).unwrap();
            }

            match (scheme, &http_proxy, &https_proxy) {
                (Scheme::Https, _, Some(proxy)) => {
                    if let Err(err) = handle.proxy(proxy) {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                }
                (_, Some(proxy), _) => {
                    if let Err(err) = handle.proxy(proxy) {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                }
                _ => {}
            }

            let mut body = Vec::new();
            envelope
                .to_writer(&mut body)
                .inspect_err(|_| {
                    send_fn_client_report_recorder
                        .record_lost_data(&envelope, LossReason::InternalError);
                })
                .expect("envelope should serialize successfully");
            let mut body = Cursor::new(body);

            let mut retry_after = None;
            let mut sentry_header = None;
            let mut headers = curl::easy::List::new();
            headers.append(&format!("X-Sentry-Auth: {auth}")).unwrap();
            headers.append("Expect:").unwrap();
            handle.http_headers(headers).unwrap();
            handle.upload(true).unwrap();
            handle.in_filesize(body.get_ref().len() as u64).unwrap();
            handle
                .read_function(move |buf| Ok(body.read(buf).unwrap_or(0)))
                .unwrap();
            handle.verbose(true).unwrap();
            handle
                .debug_function(move |info, data| {
                    let prefix = match info {
                        curl::easy::InfoType::HeaderIn => "< ",
                        curl::easy::InfoType::HeaderOut => "> ",
                        curl::easy::InfoType::DataOut => "",
                        _ => return,
                    };
                    sentry_debug!("curl: {}{}", prefix, String::from_utf8_lossy(data).trim());
                })
                .unwrap();

            let perform_result = {
                let mut handle = handle.transfer();
                let retry_after_setter = &mut retry_after;
                let sentry_header_setter = &mut sentry_header;
                handle
                    .header_function(move |data| {
                        if let Some((key, value)) = parse_curl_header_line(data) {
                            match key.as_str() {
                                "retry-after" => *retry_after_setter = Some(value),
                                "x-sentry-rate-limits" => *sentry_header_setter = Some(value),
                                _ => {}
                            }
                        }
                        true
                    })
                    .unwrap();
                handle.perform()
            };

            let perform_failed = perform_result.is_err();

            match handle.response_code() {
                Ok(response_code) => {
                    if let Some(sentry_header) = sentry_header {
                        rl.update_from_sentry_header(&sentry_header);
                    } else if let Some(retry_after) = retry_after {
                        rl.update_from_retry_after(&retry_after);
                    } else if response_code == HTTP_RATE_LIMIT_STATUS {
                        rl.update_from_429();
                    }
                    if response_code == HTTP_PAYLOAD_TOO_LARGE as u32 {
                        sentry_debug!("{HTTP_PAYLOAD_TOO_LARGE_MESSAGE}");
                    }

                    if (400..=599).contains(&response_code)
                        && response_code != HTTP_RATE_LIMIT_STATUS
                    {
                        // The server returned an HTTP error response, so the envelope was rejected
                        // at the HTTP layer even if curl also reported a transfer error.
                        send_fn_client_report_recorder
                            .record_lost_data(&envelope, LossReason::SendError);
                    } else if perform_failed && response_code == 0 {
                        // curl documents `CURLINFO_RESPONSE_CODE` as zero when no server response
                        // code has been received. If `perform` also failed, this means the send
                        // failed before an HTTP status was available, which is a network error.
                        send_fn_client_report_recorder
                            .record_lost_data(&envelope, LossReason::NetworkError);
                    }
                }
                Err(err) => {
                    sentry_debug!("Failed to send envelope: {}", err);
                    let reason = if perform_failed {
                        // `response_code` only errors when `CURLINFO_RESPONSE_CODE` is not
                        // supported. If `perform` failed too, treat the loss as the transfer error.
                        LossReason::NetworkError
                    } else {
                        LossReason::SendError
                    };
                    send_fn_client_report_recorder.record_lost_data(&envelope, reason);
                }
            }
        };

        let thread = TransportThreadOptions::new(send_fn)
            .with_client_report_recorder(client_report_recorder)
            .spawn_thread();
        Self { thread }
    }
}

impl Transport for CurlHttpTransport {
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

impl From<TransportOptions> for CurlHttpTransportOptions {
    #[inline]
    fn from(value: TransportOptions) -> Self {
        Self {
            general_options: value,
            client: None,
        }
    }
}

impl CurlHttpTransportOptions {
    /// Specify the [`CurlClient`] for the [`CurlHttpTransport`].
    #[inline]
    pub fn with_client(self, client: CurlClient) -> Self {
        let client = Some(client);
        Self { client, ..self }
    }

    /// Create a [`CurlHttpTransport`] using these options.
    #[inline]
    pub fn build(self) -> CurlHttpTransport {
        CurlHttpTransport::with_options(self)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_curl_header_line;

    #[test]
    fn parses_sentry_rate_limits_with_categories() {
        // Direct regression for issue #1111.
        let (key, value) =
            parse_curl_header_line(b"X-Sentry-Rate-Limits: 120:error:project:reason\r\n").unwrap();
        assert_eq!(key, "x-sentry-rate-limits");
        assert_eq!(value, "120:error:project:reason");
    }

    #[test]
    fn parses_sentry_rate_limits_multi_group_comma() {
        let (key, value) = parse_curl_header_line(
            b"X-Sentry-Rate-Limits: 60:transaction:key, 2700:default;error;security:organization\r\n",
        )
        .unwrap();
        assert_eq!(key, "x-sentry-rate-limits");
        assert_eq!(
            value,
            "60:transaction:key, 2700:default;error;security:organization"
        );
    }

    #[test]
    fn parses_sentry_rate_limits_empty_categories() {
        let (key, value) =
            parse_curl_header_line(b"x-sentry-rate-limits: 60::organization\r\n").unwrap();
        assert_eq!(key, "x-sentry-rate-limits");
        assert_eq!(value, "60::organization");
    }

    #[test]
    fn parses_retry_after_integer() {
        let (key, value) = parse_curl_header_line(b"Retry-After: 60\r\n").unwrap();
        assert_eq!(key, "retry-after");
        assert_eq!(value, "60");
    }

    #[test]
    fn parses_retry_after_http_date() {
        // Before the fix this was truncated to "Wed, 21 Oct 2015 07".
        let (key, value) =
            parse_curl_header_line(b"Retry-After: Wed, 21 Oct 2015 07:28:00 GMT\r\n").unwrap();
        assert_eq!(key, "retry-after");
        assert_eq!(value, "Wed, 21 Oct 2015 07:28:00 GMT");
    }

    #[test]
    fn strips_trailing_cr_only() {
        let (_, value) = parse_curl_header_line(b"Retry-After: 5\r").unwrap();
        assert_eq!(value, "5");
    }

    #[test]
    fn strips_trailing_lf_only() {
        let (_, value) = parse_curl_header_line(b"Retry-After: 5\n").unwrap();
        assert_eq!(value, "5");
    }

    #[test]
    fn lowercases_and_trims_unrelated_header() {
        let (key, value) = parse_curl_header_line(b"Content-Length: 42\r\n").unwrap();
        assert_eq!(key, "content-length");
        assert_eq!(value, "42");
    }

    #[test]
    fn trims_leading_and_inner_whitespace() {
        let (key, value) = parse_curl_header_line(b"  X-Sentry-Rate-Limits :  120 \r\n").unwrap();
        assert_eq!(key, "x-sentry-rate-limits");
        assert_eq!(value, "120");
    }

    #[test]
    fn returns_none_for_empty_line() {
        assert!(parse_curl_header_line(b"\r\n").is_none());
    }

    #[test]
    fn returns_none_for_line_without_colon() {
        // "X-Sentry-Rate-Limits\r\n" (no colon) — old code's iter.next() on a
        // one-element split iterator returned None, so the setter was untouched.
        // The new helper preserves that behavior by returning None for the
        // whole call.
        assert!(parse_curl_header_line(b"X-Sentry-Rate-Limits\r\n").is_none());
    }

    #[test]
    fn returns_none_for_non_utf8_input() {
        assert!(parse_curl_header_line(&[0xFF, 0xFE, 0xFD]).is_none());
    }
}
