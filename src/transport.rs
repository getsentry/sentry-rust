use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

use httpdate::parse_http_date;
use reqwest::header::RETRY_AFTER;
use reqwest::{Client, Proxy};

use api::protocol::Event;
use client::ClientOptions;
use Dsn;

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
    fn clone_factory(&self) -> Box<TransportFactory>;
}

impl<T: 'static + TransportFactory + Clone> InternalTransportFactoryClone for T {
    fn clone_factory(&self) -> Box<TransportFactory> {
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
    fn create_transport(&self, options: &ClientOptions) -> Box<Transport>;
}

impl<F> TransportFactory for F
where
    F: Fn(&ClientOptions) -> Box<Transport> + Clone + Send + Sync + 'static,
{
    fn create_transport(&self, options: &ClientOptions) -> Box<Transport> {
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
    fn create_transport(&self, options: &ClientOptions) -> Box<Transport> {
        let _options = options;
        Box::new(self.clone())
    }
}

/// Creates the default HTTP transport.
///
/// This is the default value for `transport` on the client options.  It
/// creates a `HttpTransport`.
#[derive(Clone)]
pub struct DefaultTransportFactory;

impl TransportFactory for DefaultTransportFactory {
    fn create_transport(&self, options: &ClientOptions) -> Box<Transport> {
        Box::new(HttpTransport::new(options))
    }
}

/// A transport can send events via HTTP to sentry.
#[derive(Debug)]
pub struct HttpTransport {
    dsn: Dsn,
    sender: Mutex<SyncSender<Option<Event<'static>>>>,
    shutdown_signal: Arc<Condvar>,
    shutdown_immediately: Arc<AtomicBool>,
    queue_size: Arc<Mutex<usize>>,
    _handle: Option<JoinHandle<()>>,
}

fn parse_retry_after(s: &str) -> Option<SystemTime> {
    if let Ok(value) = s.parse::<f64>() {
        Some(SystemTime::now() + Duration::from_secs(value.ceil() as u64))
    } else if let Ok(value) = parse_http_date(s) {
        Some(value)
    } else {
        None
    }
}

fn spawn_http_sender(
    client: Client,
    receiver: Receiver<Option<Event<'static>>>,
    dsn: Dsn,
    signal: Arc<Condvar>,
    shutdown_immediately: Arc<AtomicBool>,
    queue_size: Arc<Mutex<usize>>,
    user_agent: String,
) -> JoinHandle<()> {
    let mut disabled = SystemTime::now();

    thread::spawn(move || {
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
            if disabled > SystemTime::now() {
                continue;
            }

            if let Ok(resp) = client
                .post(url.as_str())
                .json(&event)
                .header("X-Sentry-Auth", dsn.to_auth(Some(&user_agent)).to_string())
                .send()
            {
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

            let mut size = queue_size.lock().unwrap();
            *size -= 1;
            if *size == 0 {
                signal.notify_all();
            }
        }
    })
}

impl HttpTransport {
    /// Creates a new transport.
    pub fn new(options: &ClientOptions) -> HttpTransport {
        let dsn = options.dsn.clone().unwrap();
        let user_agent = options.user_agent.to_string();
        let http_proxy = options.http_proxy.as_ref().map(|x| x.to_string());
        let https_proxy = options.https_proxy.as_ref().map(|x| x.to_string());

        let (sender, receiver) = sync_channel(30);
        let shutdown_signal = Arc::new(Condvar::new());
        let shutdown_immediately = Arc::new(AtomicBool::new(false));
        #[cfg_attr(feature = "cargo-clippy", allow(mutex_atomic))]
        let queue_size = Arc::new(Mutex::new(0));
        let mut client = Client::builder();
        if let Some(url) = http_proxy {
            client = client.proxy(Proxy::http(&url).unwrap());
        };
        if let Some(url) = https_proxy {
            client = client.proxy(Proxy::https(&url).unwrap());
        };
        let _handle = Some(spawn_http_sender(
            client.build().unwrap(),
            receiver,
            dsn.clone(),
            shutdown_signal.clone(),
            shutdown_immediately.clone(),
            queue_size.clone(),
            user_agent,
        ));
        HttpTransport {
            dsn,
            sender: Mutex::new(sender),
            shutdown_signal,
            shutdown_immediately,
            queue_size,
            _handle,
        }
    }
}

impl Transport for HttpTransport {
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

impl Drop for HttpTransport {
    fn drop(&mut self) {
        sentry_debug!("dropping http transport");
        self.shutdown_immediately.store(true, Ordering::SeqCst);
        if let Ok(sender) = self.sender.lock() {
            sender.send(None).ok();
        }
    }
}
