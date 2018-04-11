use std::time::Duration;
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use reqwest::{Client, header::Headers};
use uuid::Uuid;

use constants::USER_AGENT;
use Dsn;
use protocol::Event;

/// A transport can send rust events.
#[derive(Debug)]
pub struct Transport {
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
) -> JoinHandle<()> {
    let client = Client::new();
    thread::spawn(move || {
        let url = dsn.store_api_url().to_string();
        // TODO: if queue is full this shuts down
        while let Some(event) = receiver.recv().unwrap_or(None) {
            let auth = dsn.to_auth(Some(&USER_AGENT));
            let mut headers = Headers::new();
            headers.set_raw("X-Sentry-Auth", auth.to_string());
            // TODO: what to do with network failures. retry!
            client
                .post(url.as_str())
                .json(&event)
                .headers(headers)
                .send()
                .ok();

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
    pub fn new(dsn: &Dsn) -> Transport {
        let (sender, receiver) = sync_channel(20);
        let drain_signal = Arc::new(Condvar::new());
        let queue_size = Arc::new(Mutex::new(0));
        let handle = Some(spawn_http_sender(
            receiver,
            dsn.clone(),
            drain_signal.clone(),
            queue_size.clone(),
        ));
        Transport {
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
        // ignore the error on shutdown
        *self.queue_size.lock().unwrap() += 1;
        self.sender.lock().unwrap().send(Some(event)).ok();
        event_id
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
