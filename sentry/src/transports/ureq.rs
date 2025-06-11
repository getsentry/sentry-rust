use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "native-tls")]
use native_tls::TlsConnector;
#[cfg(feature = "rustls")]
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
#[cfg(feature = "rustls")]
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature};
#[cfg(feature = "rustls")]
use rustls::pki_types::{CertificateDer, ServerName, TrustAnchor, UnixTime};
#[cfg(feature = "rustls")]
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore};
use ureq::{Agent, AgentBuilder, Proxy};
#[cfg(feature = "rustls")]
use webpki_roots::TLS_SERVER_ROOTS;

use super::thread::TransportThread;

use crate::{sentry_debug, types::Scheme, ClientOptions, Envelope, Transport};

/// A [`Transport`] that sends events via the [`ureq`] library.
///
/// This is enabled by the `ureq` feature flag.
#[cfg_attr(doc_cfg, doc(cfg(feature = "ureq")))]
pub struct UreqHttpTransport {
    thread: TransportThread,
}

impl UreqHttpTransport {
    /// Creates a new Transport.
    pub fn new(options: &ClientOptions) -> Self {
        sentry_debug!("[UreqHttpTransport] Creating new ureq transport");
        Self::new_internal(options, None)
    }

    /// Creates a new Transport that uses the specified [`ureq::Agent`].
    pub fn with_agent(options: &ClientOptions, agent: Agent) -> Self {
        sentry_debug!("[UreqHttpTransport] Creating ureq transport with custom agent");
        Self::new_internal(options, Some(agent))
    }

    fn new_internal(options: &ClientOptions, agent: Option<Agent>) -> Self {
        let dsn = options.dsn.as_ref().unwrap();
        let scheme = dsn.scheme();
        sentry_debug!("[UreqHttpTransport] Setting up transport for DSN scheme: {:?}", scheme);
        
        let agent = agent.unwrap_or_else(|| {
            sentry_debug!("[UreqHttpTransport] Creating default ureq agent");
            let mut builder = AgentBuilder::new();

            #[cfg(feature = "native-tls")]
            {
                sentry_debug!("[UreqHttpTransport] Configuring native-tls");
                let mut tls_connector_builder = TlsConnector::builder();

                if options.accept_invalid_certs {
                    sentry_debug!("[UreqHttpTransport] Accepting invalid certificates");
                    tls_connector_builder.danger_accept_invalid_certs(true);
                }

                builder = builder.tls_connector(Arc::new(tls_connector_builder.build().unwrap()));
            }

            if options.accept_invalid_certs {
                #[cfg(feature = "rustls")]
                {
                    sentry_debug!("[UreqHttpTransport] Configuring rustls with invalid cert acceptance");
                    #[derive(Debug)]
                    struct NoVerifier;

                    impl ServerCertVerifier for NoVerifier {
                        fn verify_server_cert(
                            &self,
                            _end_entity: &CertificateDer<'_>,
                            _intermediates: &[CertificateDer<'_>],
                            _server_name: &ServerName<'_>,
                            _ocsp: &[u8],
                            _now: UnixTime,
                        ) -> Result<ServerCertVerified, rustls::Error> {
                            Ok(ServerCertVerified::assertion())
                        }

                        fn verify_tls12_signature(
                            &self,
                            message: &[u8],
                            cert: &CertificateDer<'_>,
                            dss: &DigitallySignedStruct,
                        ) -> Result<HandshakeSignatureValid, rustls::Error>
                        {
                            verify_tls12_signature(
                                message,
                                cert,
                                dss,
                                &rustls::crypto::ring::default_provider()
                                    .signature_verification_algorithms,
                            )
                        }

                        fn verify_tls13_signature(
                            &self,
                            message: &[u8],
                            cert: &CertificateDer<'_>,
                            dss: &DigitallySignedStruct,
                        ) -> Result<HandshakeSignatureValid, rustls::Error>
                        {
                            verify_tls13_signature(
                                message,
                                cert,
                                dss,
                                &rustls::crypto::ring::default_provider()
                                    .signature_verification_algorithms,
                            )
                        }

                        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
                            rustls::crypto::ring::default_provider()
                                .signature_verification_algorithms
                                .supported_schemes()
                        }
                    }

                    let mut root_store = RootCertStore::empty();
                    root_store.extend(TLS_SERVER_ROOTS.iter().map(TrustAnchor::to_owned));
                    let mut config = ClientConfig::builder()
                        .with_root_certificates(root_store)
                        .with_no_client_auth();
                    config
                        .dangerous()
                        .set_certificate_verifier(Arc::new(NoVerifier));
                    builder = builder.tls_config(Arc::new(config));
                }
            }

            match (scheme, &options.http_proxy, &options.https_proxy) {
                (Scheme::Https, _, Some(proxy)) => {
                    sentry_debug!("[UreqHttpTransport] Configuring HTTPS proxy: {}", proxy);
                    match Proxy::new(proxy) {
                        Ok(proxy) => {
                            builder = builder.proxy(proxy);
                            sentry_debug!("[UreqHttpTransport] HTTPS proxy configured successfully");
                        }
                        Err(err) => {
                            sentry_debug!("[UreqHttpTransport] Invalid HTTPS proxy: {:?}", err);
                        }
                    }
                },
                (_, Some(proxy), _) => {
                    sentry_debug!("[UreqHttpTransport] Configuring HTTP proxy: {}", proxy);
                    match Proxy::new(proxy) {
                        Ok(proxy) => {
                            builder = builder.proxy(proxy);
                            sentry_debug!("[UreqHttpTransport] HTTP proxy configured successfully");
                        }
                        Err(err) => {
                            sentry_debug!("[UreqHttpTransport] Invalid HTTP proxy: {:?}", err);
                        }
                    }
                },
                _ => {
                    sentry_debug!("[UreqHttpTransport] No proxy configuration");
                }
            }

            builder.build()
        });
        let user_agent = options.user_agent.clone();
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();
        
        sentry_debug!("[UreqHttpTransport] Target URL: {}", url);
        sentry_debug!("[UreqHttpTransport] User-Agent: {}", user_agent);

        let thread = TransportThread::new(move |envelope, rl| {
            sentry_debug!("[UreqHttpTransport] Sending envelope to Sentry");
            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            
            sentry_debug!("[UreqHttpTransport] Envelope serialized, size: {} bytes", body.len());
            
            let request = agent
                .post(&url)
                .set("X-Sentry-Auth", &auth)
                .send_bytes(&body);

            match request {
                Ok(response) => {
                    let status = response.status();
                    sentry_debug!("[UreqHttpTransport] Received response with status: {}", status);
                    
                    if let Some(sentry_header) = response.header("x-sentry-rate-limits") {
                        sentry_debug!("[UreqHttpTransport] Processing rate limit header: {}", sentry_header);
                        rl.update_from_sentry_header(sentry_header);
                    } else if let Some(retry_after) = response.header("retry-after") {
                        sentry_debug!("[UreqHttpTransport] Processing retry-after header: {}", retry_after);
                        rl.update_from_retry_after(retry_after);
                    } else if response.status() == 429 {
                        sentry_debug!("[UreqHttpTransport] Rate limited (429), no retry-after header");
                        rl.update_from_429();
                    }

                    match response.into_string() {
                        Err(err) => {
                            sentry_debug!("[UreqHttpTransport] Failed to read sentry response: {}", err);
                        }
                        Ok(text) => {
                            sentry_debug!("[UreqHttpTransport] Get response: `{}`", text);
                        }
                    }
                }
                Err(err) => {
                    sentry_debug!("[UreqHttpTransport] Failed to send envelope: {}", err);
                }
            }
        });
        
        sentry_debug!("[UreqHttpTransport] Transport thread created successfully");
        Self { thread }
    }
}

impl Transport for UreqHttpTransport {
    fn send_envelope(&self, envelope: Envelope) {
        sentry_debug!("[UreqHttpTransport] Queueing envelope for sending");
        self.thread.send(envelope)
    }
    
    fn flush(&self, timeout: Duration) -> bool {
        sentry_debug!("[UreqHttpTransport] Flushing transport (timeout: {}ms)", timeout.as_millis());
        let result = self.thread.flush(timeout);
        sentry_debug!("[UreqHttpTransport] Flush completed: {}", if result { "success" } else { "timeout" });
        result
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        sentry_debug!("[UreqHttpTransport] Shutting down transport (timeout: {}ms)", timeout.as_millis());
        let result = self.flush(timeout);
        sentry_debug!("[UreqHttpTransport] Shutdown completed: {}", if result { "success" } else { "timeout" });
        result
    }
}
