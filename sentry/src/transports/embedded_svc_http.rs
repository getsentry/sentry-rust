use sentry_core::TransportOptions;

use super::{HTTP_PAYLOAD_TOO_LARGE, HTTP_PAYLOAD_TOO_LARGE_MESSAGE};
use crate::{sentry_debug, ClientOptions, Transport};
use embedded_svc::http::client::Client as HttpClient;
use esp_idf_svc::{http::client::EspHttpConnection, io::Write};

/// Transport using the embedded-svc http client
pub struct EmbeddedSVCHttpTransport {
    /// The transport options.
    ///
    /// For backwards-compatibility, this is an [`Option`]. A value of [`None`] only occurs when
    /// the transport is constructed with [`Self::new`] without a `dsn` in the [`ClientOptions`].
    options: Option<TransportOptions>,
}

/// Options for constructing an [`EmbeddedSVCHttpTransport`].
///
/// Currently, this is a wrapper around a [`TransportOptions`], and must be created with the
/// `From<TransportOptions>` implementation.
#[derive(Debug)]
pub struct EmbeddedSVCHttpTransportOptions {
    general_options: TransportOptions,
}

impl EmbeddedSVCHttpTransport {
    /// Backwards-compatible method for creating an [`EmbeddedSVCHttpTransport`].
    ///
    /// Please use [`EmbeddedSVCHttpTransportOptions::build`] instead.
    #[deprecated = "use `EmbeddedSVCHttpTransportOptions::build` instead"]
    pub fn new(options: &ClientOptions) -> Self {
        Self {
            options: TransportOptions::try_from_client_options(options),
        }
    }

    /// Creates a new [`EmbeddedSVCHttpTransport`] with the given `options`.
    pub(super) fn with_options(options: EmbeddedSVCHttpTransportOptions) -> Self {
        Self {
            options: Some(options.general_options),
        }
    }
}

impl EmbeddedSVCHttpTransport {
    fn send_envelope(
        &self,
        envelope: sentry_core::Envelope,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let TransportOptions {
            dsn, user_agent, ..
        } = self.options.as_ref().ok_or_else(|| "No DSN specified")?;
        let auth = dsn.to_auth(Some(user_agent)).to_string();
        let headers = [("X-Sentry-Auth", auth.as_str())];
        let url = dsn.envelope_api_url();

        let mut body = Vec::new();
        envelope.to_writer(&mut body)?;

        let config = esp_idf_svc::http::client::Configuration {
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        };

        let mut client = HttpClient::wrap(EspHttpConnection::new(&config)?);

        let mut request = client.post(url.as_str(), &headers)?;
        request.write_all(&body)?;
        request.flush()?;
        let mut response = request.submit()?;
        let status = response.status();

        // read the whole response
        let mut buf = [0u8; 1024];
        while response.read(&mut buf)? > 0 {}
        if status == HTTP_PAYLOAD_TOO_LARGE {
            sentry_debug!("{HTTP_PAYLOAD_TOO_LARGE_MESSAGE}");
        }

        Ok(())
    }
}

impl Transport for EmbeddedSVCHttpTransport {
    fn send_envelope(&self, envelope: sentry_core::Envelope) {
        if let Err(err) = self.send_envelope(envelope) {
            sentry_debug!("Failed to send envelope: {}", err);
        }
    }
}

impl From<TransportOptions> for EmbeddedSVCHttpTransportOptions {
    #[inline]
    fn from(value: TransportOptions) -> Self {
        Self {
            general_options: value,
        }
    }
}

impl EmbeddedSVCHttpTransportOptions {
    /// Create an [`EmbeddedSVCHttpTransport`] using these options.
    #[inline]
    pub fn build(self) -> EmbeddedSVCHttpTransport {
        EmbeddedSVCHttpTransport::with_options(self)
    }
}
