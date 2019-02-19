use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

#[cfg(any(feature = "with_reqwest_transport", feature = "with_curl_transport"))]
use httpdate::parse_http_date;

#[cfg(feature = "with_reqwest_transport")]
use reqwest::{header::RETRY_AFTER, Client, Proxy};

#[cfg(feature = "with_curl_transport")]
use {crate::internals::Scheme, curl, std::io::Read};

use crate::client::ClientOptions;
use crate::protocol::Event;

/// The trait for transports.
///
/// A transport is responsible for sending events to Sentry.  Custom implementations
/// can be created to use a different abstraction to send events.  This is for instance
/// used for the test system.
pub trait Transport: Send + Sync + 'static {
    /// Sends an event.
    fn send_event(&self, event: Event<'static>);

    /// Drains the queue if there is one.
    ///
    /// The default implementation does nothing.  If the queue was successfully
    /// shutdowned the return value should be `true` or `false` if events were
    /// left in it.
    fn shutdown(&self, timeout: Duration) -> bool {
        let _timeout = timeout;
        true
    }
}

pub trait InternalTransportFactoryClone {
    fn clone_factory(&self) -> Box<dyn TransportFactory>;
}

impl<T: 'static + TransportFactory + Clone> InternalTransportFactoryClone for T {
    fn clone_factory(&self) -> Box<dyn TransportFactory> {
        Box::new(self.clone())
    }
}

/// A factory creating transport instances.
///
/// Because options are potentially reused between different clients the
/// options do not actually contain a transport but a factory object that
/// can create transports instead.
///
/// The factory has a single method that creates a new boxed transport.
/// Because transports can be wrapped in `Arc`s and those are clonable
/// any `Arc<Transport>` is also a valid transport factory.  This for
/// instance lets you put a `Arc<TestTransport>` directly into the options.
///
/// This is automatically implemented for all closures optionally taking
/// options and returning a boxed factory.
pub trait TransportFactory: Send + Sync + InternalTransportFactoryClone {
    /// Given some options creates a transport.
    fn create_transport(&self, options: &ClientOptions) -> Box<dyn Transport>;
}

impl<F> TransportFactory for F
where
    F: Fn(&ClientOptions) -> Box<dyn Transport> + Clone + Send + Sync + 'static,
{
    fn create_transport(&self, options: &ClientOptions) -> Box<dyn Transport> {
        (*self)(options)
    }
}

impl<T: Transport> Transport for Arc<T> {
    fn send_event(&self, event: Event<'static>) {
        (**self).send_event(event)
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        (**self).shutdown(timeout)
    }
}

impl<T: Transport> TransportFactory for Arc<T> {
    fn create_transport(&self, options: &ClientOptions) -> Box<dyn Transport> {
        let _options = options;
        Box::new(self.clone())
    }
}

/// Creates the default HTTP transport.
///
/// This is the default value for `transport` on the client options.  It
/// creates a `HttpTransport`.  If no http transport was compiled into the
/// library it will panic on transport creation.
#[derive(Clone)]
pub struct DefaultTransportFactory;

impl TransportFactory for DefaultTransportFactory {
    fn create_transport(&self, options: &ClientOptions) -> Box<dyn Transport> {
        #[cfg(any(feature = "with_reqwest_transport", feature = "with_curl_transport"))]
        {
            Box::new(HttpTransport::new(options))
        }
        #[cfg(not(any(feature = "with_reqwest_transport", feature = "with_curl_transport")))]
        {
            panic!("sentry crate was compiled without transport")
        }
    }
}

#[cfg(any(feature = "with_reqwest_transport", feature = "with_curl_transport"))]
fn parse_retry_after(s: &str) -> Option<SystemTime> {
    if let Ok(value) = s.parse::<f64>() {
        Some(SystemTime::now() + Duration::from_secs(value.ceil() as u64))
    } else if let Ok(value) = parse_http_date(s) {
        Some(value)
    } else {
        None
    }
}

macro_rules! implement_http_transport {
    (
        $(#[$attr:meta])*
        pub struct $typename:ident;
        fn spawn($($argname:ident: $argty:ty,)*) $body:block
    ) => {
        $(#[$attr])*
        pub struct $typename {
            sender: Mutex<SyncSender<Option<Event<'static>>>>,
            shutdown_signal: Arc<Condvar>,
            shutdown_immediately: Arc<AtomicBool>,
            queue_size: Arc<Mutex<usize>>,
            _handle: Option<JoinHandle<()>>,
        }

        impl $typename {
            /// Creates a new transport.
            pub fn new(options: &ClientOptions) -> $typename {
                fn spawn($($argname: $argty,)*) -> JoinHandle<()> { $body }

                let (sender, receiver) = sync_channel(30);
                let shutdown_signal = Arc::new(Condvar::new());
                let shutdown_immediately = Arc::new(AtomicBool::new(false));
                #[allow(clippy::mutex_atomic)]
                let queue_size = Arc::new(Mutex::new(0));
                let _handle = Some(spawn(
                    options,
                    receiver,
                    shutdown_signal.clone(),
                    shutdown_immediately.clone(),
                    queue_size.clone(),
                ));
                $typename {
                    sender: Mutex::new(sender),
                    shutdown_signal,
                    shutdown_immediately,
                    queue_size,
                    _handle,
                }
            }
        }

        impl Transport for $typename {
            fn send_event(&self, event: Event<'static>) {
                // we count up before we put the item on the queue and in case the
                // queue is filled with too many items or we shut down, we decrement
                // the count again as there is nobody that can pick it up.
                *self.queue_size.lock().unwrap() += 1;
                if self.sender.lock().unwrap().try_send(Some(event)).is_err() {
                    *self.queue_size.lock().unwrap() -= 1;
                }
            }

            fn shutdown(&self, timeout: Duration) -> bool {
                sentry_debug!("shutting down http transport");
                let guard = self.queue_size.lock().unwrap();
                if *guard == 0 {
                    true
                } else {
                    if let Ok(sender) = self.sender.lock() {
                        sender.send(None).ok();
                    }
                    self.shutdown_signal.wait_timeout(guard, timeout).is_ok()
                }
            }
        }

        impl Drop for $typename {
            fn drop(&mut self) {
                sentry_debug!("dropping http transport");
                self.shutdown_immediately.store(true, Ordering::SeqCst);
                if let Ok(sender) = self.sender.lock() {
                    sender.send(None).ok();
                }
            }
        }
    }
}

#[cfg(feature = "with_reqwest_transport")]
implement_http_transport! {
    /// A transport can send events via HTTP to sentry via `reqwest`.
    ///
    /// When the `with_default_transport` feature is enabled this will currently
    /// be the default transport.  This is separately enabled by the
    /// `with_reqwest_transport` flag.
    pub struct ReqwestHttpTransport;

    fn spawn(
        options: &ClientOptions,
        receiver: Receiver<Option<Event<'static>>>,
        signal: Arc<Condvar>,
        shutdown_immediately: Arc<AtomicBool>,
        queue_size: Arc<Mutex<usize>>,
    ) {
        let dsn = options.dsn.clone().unwrap();
        let user_agent = options.user_agent.to_string();
        let http_proxy = options.http_proxy.as_ref().map(|x| x.to_string());
        let https_proxy = options.https_proxy.as_ref().map(|x| x.to_string());
        let mut client = Client::builder();
        if let Some(url) = http_proxy {
            client = client.proxy(Proxy::http(&url).unwrap());
        };
        if let Some(url) = https_proxy {
            client = client.proxy(Proxy::https(&url).unwrap());
        };
        let client = client.build().unwrap();

        let mut disabled = SystemTime::now();

        thread::Builder::new()
            .name("sentry-transport".to_string())
            .spawn(move || {
                sentry_debug!("spawning reqwest transport");
                let url = dsn.store_api_url().to_string();

                while let Some(event) = receiver.recv().unwrap_or(None) {
                    // on drop we want to not continue processing the queue.
                    if shutdown_immediately.load(Ordering::SeqCst) {
                        let mut size = queue_size.lock().unwrap();
                        *size = 0;
                        signal.notify_all();
                        break;
                    }

                    // while we are disabled due to rate limits, skip
                    let now = SystemTime::now();
                    if let Ok(time_left) = disabled.duration_since(now) {
                        sentry_debug!(
                            "Skipping event send because we're disabled due to rate limits for {}s",
                            time_left.as_secs()
                        );
                        continue;
                    }

                    match client
                        .post(url.as_str())
                        .json(&event)
                        .header("X-Sentry-Auth", dsn.to_auth(Some(&user_agent)).to_string())
                        .send()
                    {
                        Ok(resp) => {
                            if resp.status() == 429 {
                                if let Some(retry_after) = resp
                                    .headers()
                                    .get(RETRY_AFTER)
                                    .and_then(|x| x.to_str().ok())
                                    .and_then(parse_retry_after)
                                {
                                    disabled = retry_after;
                                }
                            }
                        }
                        Err(err) => {
                            sentry_debug!("Failed to send event: {}", err);
                        }
                    }

                    let mut size = queue_size.lock().unwrap();
                    *size -= 1;
                    if *size == 0 {
                        signal.notify_all();
                    }
                }
            }).unwrap()
    }
}

#[cfg(feature = "with_curl_transport")]
implement_http_transport! {
    /// A transport can send events via HTTP to sentry via `curl`.
    ///
    /// This is enabled by the `with_curl_transport` flag.
    pub struct CurlHttpTransport;

    fn spawn(
        options: &ClientOptions,
        receiver: Receiver<Option<Event<'static>>>,
        signal: Arc<Condvar>,
        shutdown_immediately: Arc<AtomicBool>,
        queue_size: Arc<Mutex<usize>>,
    ) {
        let dsn = options.dsn.clone().unwrap();
        let user_agent = options.user_agent.to_string();
        let http_proxy = options.http_proxy.as_ref().map(|x| x.to_string());
        let https_proxy = options.https_proxy.as_ref().map(|x| x.to_string());

        let mut disabled = SystemTime::now();
        let mut handle = curl::easy::Easy::new();

        thread::spawn(move || {
            sentry_debug!("spawning curl transport");
            let url = dsn.store_api_url().to_string();

            while let Some(event) = receiver.recv().unwrap_or(None) {
                // on drop we want to not continue processing the queue.
                if shutdown_immediately.load(Ordering::SeqCst) {
                    let mut size = queue_size.lock().unwrap();
                    *size = 0;
                    signal.notify_all();
                    break;
                }

                // while we are disabled due to rate limits, skip
                let now = SystemTime::now();
                if let Ok(time_left) = disabled.duration_since(now) {
                    sentry_debug!(
                        "Skipping event send because we're disabled due to rate limits for {}s",
                        time_left.as_secs()
                    );
                    continue;
                }

                handle.reset();
                handle.url(&url).unwrap();
                handle.custom_request("POST").unwrap();

                match (dsn.scheme(), &http_proxy, &https_proxy) {
                    (Scheme::Https, _, &Some(ref proxy)) => {
                        handle.proxy(&proxy).unwrap();
                    }
                    (_, &Some(ref proxy), _) => {
                        handle.proxy(&proxy).unwrap();
                    }
                    _ => {}
                }

                let body = serde_json::to_vec(&event).unwrap();
                let mut retry_after = None;
                let mut headers = curl::easy::List::new();
                headers.append(&format!("X-Sentry-Auth: {}", dsn.to_auth(Some(&user_agent)))).unwrap();
                headers.append("Expect:").unwrap();
                headers.append("Content-Type: application/json").unwrap();
                handle.http_headers(headers).unwrap();
                handle.upload(true).unwrap();
                handle.in_filesize(body.len() as u64).unwrap();
                handle.read_function(move |buf| Ok((&body[..]).read(buf).unwrap_or(0))).unwrap();
                handle.verbose(true).unwrap();
                handle.debug_function(move |info, data| {
                    let prefix = match info {
                        curl::easy::InfoType::HeaderIn => "< ",
                        curl::easy::InfoType::HeaderOut => "> ",
                        curl::easy::InfoType::DataOut => "",
                        _ => return
                    };
                    sentry_debug!("curl: {}{}", prefix, String::from_utf8_lossy(data).trim());
                }).unwrap();

                {
                    let mut handle = handle.transfer();
                    let retry_after_setter = &mut retry_after;
                    handle.header_function(move |data| {
                        if let Ok(data) = std::str::from_utf8(data) {
                            let mut iter = data.split(':');
                            if let Some(key) = iter.next().map(|x| x.to_lowercase()) {
                                if key == "retry-after" {
                                    *retry_after_setter = iter.next().map(|x| x.trim().to_string());
                                }
                            }
                        }
                        true
                    }).unwrap();
                    handle.perform().ok();
                }

                match handle.response_code() {
                    Ok(429) => {
                        if let Some(retry_after) = retry_after
                            .as_ref()
                            .map(|x| x.as_str())
                            .and_then(parse_retry_after)
                        {
                            disabled = retry_after;
                        }
                    }
                    Ok(200) | Ok(201) => {}
                    _ => {
                        sentry_debug!("Failed to send event");
                    }
                }

                let mut size = queue_size.lock().unwrap();
                *size -= 1;
                if *size == 0 {
                    signal.notify_all();
                }
            }
        })
    }
}

#[cfg(feature = "with_reqwest_transport")]
type DefaultTransport = ReqwestHttpTransport;

#[cfg(all(
    feature = "with_curl_transport",
    not(feature = "with_reqwest_transport")
))]
type DefaultTransport = CurlHttpTransport;

/// The default http transport.
pub type HttpTransport = DefaultTransport;
