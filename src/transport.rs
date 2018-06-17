use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use reqwest::header::{Headers, RetryAfter};
use reqwest::{Client, StatusCode};

use api::protocol::Event;
use Dsn;

#[derive(Debug)]
struct RealTransportImpl {
    sender: Mutex<SyncSender<Option<Event<'static>>>>,
    drain_signal: Arc<Condvar>,
    queue_size: Arc<Mutex<usize>>,
    _handle: Option<JoinHandle<()>>,
}

#[cfg(any(test, feature = "with_test_support"))]
#[derive(Debug)]
struct TestTransportImpl {
    collected: Mutex<Vec<Event<'static>>>,
}

#[derive(Debug)]
enum TransportImpl {
    Real(RealTransportImpl),
    #[cfg(any(test, feature = "with_test_support"))]
    Test(TestTransportImpl),
}


/// A transport can send rust events.
#[derive(Debug)]
pub struct Transport {
    dsn: Dsn,
    inner: TransportImpl,
}

fn spawn_http_sender(
    receiver: Receiver<Option<Event<'static>>>,
    dsn: Dsn,
    signal: Arc<Condvar>,
    queue_size: Arc<Mutex<usize>>,
    user_agent: String,
) -> JoinHandle<()> {
    let client = Client::new();
    let mut disabled: Option<(Instant, RetryAfter)> = None;

    thread::spawn(move || {
        let url = dsn.store_api_url().to_string();

        while let Some(event) = receiver.recv().unwrap_or(None) {
            // while we are disabled due to rate limits, skip
            match disabled {
                Some((disabled_at, RetryAfter::Delay(disabled_for))) => {
                    if disabled_at.elapsed() > disabled_for {
                        disabled = None;
                    } else {
                        continue;
                    }
                }
                Some((_, RetryAfter::DateTime(wait_until))) => {
                    if SystemTime::from(wait_until) > SystemTime::now() {
                        disabled = None;
                    } else {
                        continue;
                    }
                }
                None => {}
            }

            let auth = dsn.to_auth(Some(&user_agent));
            let mut headers = Headers::new();
            headers.set_raw("X-Sentry-Auth", auth.to_string());

            if let Ok(resp) = client
                .post(url.as_str())
                .json(&event)
                .headers(headers)
                .send()
            {
                if resp.status() == StatusCode::TooManyRequests {
                    disabled = resp.headers()
                        .get::<RetryAfter>()
                        .map(|x| (Instant::now(), *x));
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

impl Transport {
    /// Creates a new transport.
    pub fn new(dsn: Dsn, user_agent: String) -> Transport {
        let (sender, receiver) = sync_channel(30);
        let drain_signal = Arc::new(Condvar::new());
        #[cfg_attr(feature = "cargo-clippy", allow(mutex_atomic))]
        let queue_size = Arc::new(Mutex::new(0));
        let _handle = Some(spawn_http_sender(
            receiver,
            dsn.clone(),
            drain_signal.clone(),
            queue_size.clone(),
            user_agent,
        ));
        Transport {
            dsn,
            inner: TransportImpl::Real(RealTransportImpl {
                sender: Mutex::new(sender),
                drain_signal,
                queue_size,
                _handle,
            }),
        }
    }

    /// Creates a transport for testing.
    #[cfg(any(test, feature = "with_test_support"))]
    pub fn testable(dsn: Dsn) -> Transport {
        Transport {
            dsn,
            inner: TransportImpl::Test(TestTransportImpl {
                collected: Mutex::new(vec![])
            }),
        }
    }

    /// Returns the dsn of the transport
    pub fn dsn(&self) -> &Dsn {
        &self.dsn
    }

    /// Sends a sentry event and return the event ID.
    pub fn send_event(&self, event: Event<'static>) {
        match self.inner {
            TransportImpl::Real(ref ti) => {
                // we count up before we put the item on the queue and in case the
                // queue is filled with too many items or we shut down, we decrement
                // the count again as there is nobody that can pick it up.
                *ti.queue_size.lock().unwrap() += 1;
                if ti.sender.lock().unwrap().try_send(Some(event)).is_err() {
                    *ti.queue_size.lock().unwrap() -= 1;
                }
            }
            #[cfg(any(test, feature = "with_test_support"))]
            TransportImpl::Test(ref ti) => {
                ti.collected.lock().unwrap().push(event);
            }
        }
    }

    /// Drains remaining messages in the transport.
    ///
    /// This returns `true` if the queue was successfully drained in the
    /// given time or `false` if not.
    pub fn drain(&self, timeout: Option<Duration>) -> bool {
        match self.inner {
            TransportImpl::Real(ref ti) => {
                let guard = ti.queue_size.lock().unwrap();
                if *guard == 0 {
                    return true;
                }
                if let Some(timeout) = timeout {
                    ti.drain_signal.wait_timeout(guard, timeout).is_ok()
                } else {
                    ti.drain_signal.wait(guard).is_ok()
                }
            }
            #[cfg(any(test, feature = "with_test_support"))]
            TransportImpl::Test(..) => true
        }
    }

    /// Returns all events currently in the transport.
    ///
    /// Only available for the testable client.
    #[cfg(any(test, feature = "with_test_support"))]
    pub fn fetch_and_clear_events(&self) -> Vec<Event<'static>> {
        match self.inner {
            TransportImpl::Real(..) => {
                panic!("Can only fetch events from testable transports");
            },
            #[cfg(any(test, feature = "with_test_support"))]
            TransportImpl::Test(ref ti) => {
                use std::mem;
                let mut guard = ti.collected.lock().unwrap();
                mem::replace(&mut *guard, vec![])
            }
        }
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        match self.inner {
            TransportImpl::Real(ref ti) => {
                if let Ok(sender) = ti.sender.lock() {
                    sender.send(None).ok();
                }
            }
            #[cfg(any(test, feature = "with_test_support"))]
            TransportImpl::Test(..) => {}
        }
    }
}
