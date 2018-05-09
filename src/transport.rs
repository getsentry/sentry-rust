use std::time::{Duration, Instant, SystemTime};
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use reqwest::{Client, StatusCode};
use reqwest::header::{Headers, RetryAfter};
use uuid::Uuid;

use Dsn;
use protocol::Event;

/// A transport can send rust events.
#[derive(Debug)]
pub struct Transport {
    dsn: Dsn,
    sender: Mutex<SyncSender<Option<Event<'static>>>>,
    drain_signal: Arc<Condvar>,
    queue_size: Arc<Mutex<usize>>,
    _handle: Option<JoinHandle<()>>,
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

            if let Some(resp) = client
                .post(url.as_str())
                .json(&event)
                .headers(headers)
                .send()
                .ok()
            {
                if resp.status() == StatusCode::TooManyRequests {
                    disabled = resp.headers()
                        .get::<RetryAfter>()
                        .map(|x| (Instant::now(), x.clone()));
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
    /// Creates a new client.
    pub fn new(dsn: Dsn, user_agent: String) -> Transport {
        let (sender, receiver) = sync_channel(30);
        let drain_signal = Arc::new(Condvar::new());
        let queue_size = Arc::new(Mutex::new(0));
        let handle = Some(spawn_http_sender(
            receiver,
            dsn.clone(),
            drain_signal.clone(),
            queue_size.clone(),
            user_agent,
        ));
        Transport {
            dsn: dsn,
            sender: Mutex::new(sender),
            drain_signal: drain_signal,
            queue_size: queue_size,
            _handle: handle,
        }
    }

    /// Sends a sentry event and return the event ID.
    pub fn send_event(&self, mut event: Event<'static>) -> Uuid {
        if event.id.is_none() {
            event.id = Some(Uuid::new_v4());
        }
        let event_id = event.id.unwrap();

        // we count up before we put the item on the queue and in case the
        // queue is filled with too many items or we shut down, we decrement
        // the count again as there is nobody that can pick it up.
        *self.queue_size.lock().unwrap() += 1;
        if self.sender.lock().unwrap().try_send(Some(event)).is_err() {
            *self.queue_size.lock().unwrap() -= 1;
        }

        event_id
    }

    /// Returns the dsn of the transport
    pub fn dsn(&self) -> &Dsn {
        &self.dsn
    }

    /// Drains remaining messages in the transport.
    ///
    /// This returns `true` if the queue was successfully drained in the
    /// given time or `false` if not.
    pub fn drain(&self, timeout: Option<Duration>) -> bool {
        let guard = self.queue_size.lock().unwrap();
        if *guard == 0 {
            return true;
        }
        if let Some(timeout) = timeout {
            self.drain_signal.wait_timeout(guard, timeout).is_ok()
        } else {
            self.drain_signal.wait(guard).is_ok()
        }
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        if let Ok(sender) = self.sender.lock() {
            sender.send(None);
        }
    }
}
