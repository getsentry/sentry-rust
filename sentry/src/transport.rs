use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(feature = "reqwest")]
use reqwest_::{header as ReqwestHeaders, Client as ReqwestClient, Proxy};

#[cfg(feature = "curl")]
use crate::types::Scheme;
#[cfg(feature = "curl")]
use curl_::{self as curl, easy::Easy as CurlClient};
#[cfg(feature = "curl")]
use std::io::{Cursor, Read};

#[cfg(feature = "surf")]
use surf_::{http::headers as SurfHeaders, Client as SurfClient};

use crate::{
    ClientOptions, Envelope, RateLimiter, RateLimitingCategory, Transport, TransportFactory,
};
use sentry_core::sentry_debug;

/// Creates the default HTTP transport.
///
/// This is the default value for `transport` on the client options.  It
/// creates a `HttpTransport`.  If no http transport was compiled into the
/// library it will panic on transport creation.
#[derive(Clone)]
pub struct DefaultTransportFactory;

impl TransportFactory for DefaultTransportFactory {
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport> {
        #[cfg(any(feature = "reqwest", feature = "curl", feature = "surf"))]
        {
            Arc::new(HttpTransport::new(options))
        }
        #[cfg(not(any(feature = "reqwest", feature = "curl", feature = "surf")))]
        {
            let _ = options;
            panic!("sentry crate was compiled without transport")
        }
    }
}

enum Task {
    SendEnvelope(Envelope),
    Flush(SyncSender<()>),
    Shutdown,
}

struct TransportThread {
    sender: SyncSender<Task>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TransportThread {
    fn new<SendFn, SendFuture>(mut send: SendFn) -> Self
    where
        SendFn: FnMut(Envelope, RateLimiter) -> SendFuture + Send + 'static,
        // NOTE: returning RateLimiter here, otherwise we are in borrow hell
        SendFuture: std::future::Future<Output = RateLimiter>,
    {
        let (sender, receiver) = sync_channel(30);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_worker = shutdown.clone();
        let handle = thread::Builder::new()
            .name("sentry-transport".into())
            .spawn(move || {
                // create a runtime on the transport thread
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let mut rl = RateLimiter::new();

                // and block on an async fn in this runtime/thread
                rt.block_on(async move {
                    for task in receiver.into_iter() {
                        if shutdown_worker.load(Ordering::SeqCst) {
                            return;
                        }
                        let envelope = match task {
                            Task::SendEnvelope(envelope) => envelope,
                            Task::Flush(sender) => {
                                sender.send(()).ok();
                                continue;
                            }
                            Task::Shutdown => {
                                return;
                            }
                        };

                        if let Some(time_left) =  rl.is_disabled(RateLimitingCategory::Any) {
                                sentry_debug!(
                                    "Skipping event send because we're disabled due to rate limits for {}s",
                                    time_left.as_secs()
                                );
                                continue;
                            }
                            rl = send(envelope, rl).await;
                    }
                })
            })
            .ok();

        Self {
            sender,
            shutdown,
            handle,
        }
    }

    fn send(&self, envelope: Envelope) {
        let _ = self.sender.send(Task::SendEnvelope(envelope));
    }

    fn flush(&self, timeout: Duration) -> bool {
        let (sender, receiver) = sync_channel(1);
        let _ = self.sender.send(Task::Flush(sender));
        receiver.recv_timeout(timeout).is_err()
    }
}

impl Drop for TransportThread {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.sender.send(Task::Shutdown);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

/// A [`Transport`] that sends events via the [`reqwest`] library.
///
/// When the `transport` feature is enabled this will currently
/// be the default transport.  This is separately enabled by the
/// `reqwest` flag.
pub struct ReqwestHttpTransport {
    thread: TransportThread,
}

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

/// A [`Transport`] that sends events via the [`curl`] library.
///
/// This is enabled by the `curl` flag.
pub struct CurlHttpTransport {
    thread: TransportThread,
}

impl CurlHttpTransport {
    /// Creates a new Transport.
    pub fn new(options: &ClientOptions) -> Self {
        Self::new_internal(options, None)
    }

    /// Creates a new Transport that uses the specified [`CurlClient`].
    pub fn with_client(options: &ClientOptions, client: CurlClient) -> Self {
        Self::new_internal(options, Some(client))
    }

    fn new_internal(options: &ClientOptions, client: Option<CurlClient>) -> Self {
        let client = client.unwrap_or_else(CurlClient::new);
        let http_proxy = options.http_proxy.as_ref().map(ToString::to_string);
        let https_proxy = options.https_proxy.as_ref().map(ToString::to_string);
        let dsn = options.dsn.as_ref().unwrap();
        let user_agent = options.user_agent.to_owned();
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();
        let scheme = dsn.scheme();

        let mut handle = client;
        let thread = TransportThread::new(move |envelope, mut rl| {
            handle.reset();
            handle.url(&url).unwrap();
            handle.custom_request("POST").unwrap();

            match (scheme, &http_proxy, &https_proxy) {
                (Scheme::Https, _, &Some(ref proxy)) => {
                    handle.proxy(&proxy).unwrap();
                }
                (_, &Some(ref proxy), _) => {
                    handle.proxy(&proxy).unwrap();
                }
                _ => {}
            }

            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            let mut body = Cursor::new(body);

            let mut retry_after = None;
            let mut sentry_header = None;
            let mut headers = curl::easy::List::new();
            headers.append(&format!("X-Sentry-Auth: {}", auth)).unwrap();
            headers.append("Expect:").unwrap();
            headers.append("Content-Type: application/json").unwrap();
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

            {
                let mut handle = handle.transfer();
                let retry_after_setter = &mut retry_after;
                let sentry_header_setter = &mut sentry_header;
                handle
                    .header_function(move |data| {
                        if let Ok(data) = std::str::from_utf8(data) {
                            let mut iter = data.split(':');
                            if let Some(key) = iter.next().map(str::to_lowercase) {
                                if key == "retry-after" {
                                    *retry_after_setter = iter.next().map(|x| x.trim().to_string());
                                } else if key == "x-sentry-rate-limits" {
                                    *sentry_header_setter =
                                        iter.next().map(|x| x.trim().to_string());
                                }
                            }
                        }
                        true
                    })
                    .unwrap();
                handle.perform().ok();
            }

            match handle.response_code() {
                Ok(_) => {
                    if let Some(retry_after) = retry_after {
                        rl.update_from_retry_after(&retry_after);
                    }
                    if let Some(sentry_header) = sentry_header {
                        rl.update_from_sentry_header(&sentry_header);
                    }
                }
                Err(err) => {
                    sentry_debug!("Failed to send envelope: {}", err);
                }
            }
            async move { rl }
        });
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

/// A [`Transport`] that sends events via the [`surf`] library.
///
/// This is enabled by the `surf` flag.
pub struct SurfHttpTransport {
    thread: TransportThread,
}

impl SurfHttpTransport {
    /// Creates a new Transport.
    pub fn new(options: &ClientOptions) -> Self {
        Self::new_internal(options, None)
    }

    /// Creates a new Transport that uses the specified [`SurfClient`].
    pub fn with_client(options: &ClientOptions, client: SurfClient) -> Self {
        Self::new_internal(options, Some(client))
    }

    fn new_internal(options: &ClientOptions, client: Option<SurfClient>) -> Self {
        let client = client.unwrap_or_else(SurfClient::new);
        let dsn = options.dsn.as_ref().unwrap();
        let user_agent = options.user_agent.to_owned();
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();

        let thread = TransportThread::new(move |envelope, mut rl| {
            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            let request = client.post(&url).header("X-Sentry-Auth", &auth).body(body);

            async move {
                match request.await {
                    Ok(mut response) => {
                        if let Some(retry_after) = response
                            .header(SurfHeaders::RETRY_AFTER)
                            .map(|x| x.as_str())
                        {
                            rl.update_from_retry_after(retry_after);
                        }
                        if let Some(sentry_header) =
                            response.header("x-sentry-rate-limits").map(|x| x.as_str())
                        {
                            rl.update_from_retry_after(sentry_header);
                        }

                        match response.body_string().await {
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

impl Transport for SurfHttpTransport {
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

#[cfg(feature = "reqwest")]
type DefaultTransport = ReqwestHttpTransport;

#[cfg(all(feature = "curl", not(feature = "reqwest"), not(feature = "surf")))]
type DefaultTransport = CurlHttpTransport;

#[cfg(all(feature = "surf", not(feature = "reqwest"), not(feature = "curl")))]
type DefaultTransport = SurfHttpTransport;

/// The default http transport.
#[cfg(any(feature = "reqwest", feature = "curl", feature = "surf"))]
pub type HttpTransport = DefaultTransport;
