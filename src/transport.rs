use std::sync::{Mutex, mpsc::{channel, Receiver, Sender}};
use std::thread::{self, JoinHandle};

use reqwest::{Client, header::Headers};
use uuid::Uuid;

use constants::VERSION;
use Dsn;
use protocol::Event;

/// A transport can send rust events.
#[derive(Debug)]
pub struct Transport {
    sender: Mutex<Sender<Option<Event>>>,
    handle: Option<JoinHandle<()>>,
}

fn spawn_http_sender(receiver: Receiver<Option<Event>>, dsn: Dsn) -> JoinHandle<()> {
    let user_agent = format!("sentry-rust/{}", VERSION);
    let client = Client::new();
    thread::spawn(move || {
        let url = dsn.store_api_url().to_string();
        while let Some(event) = receiver.recv().unwrap_or(None) {
            let auth = dsn.to_auth(Some(&user_agent));
            let mut headers = Headers::new();
            headers.set_raw("X-Sentry-Auth", auth.to_string());
            client
                .post(url.as_str())
                .json(&event)
                .headers(headers)
                .send()
                .unwrap();
        }
    })
}

impl Transport {
    /// Creates a new client.
    pub fn new(dsn: &Dsn) -> Transport {
        let (sender, receiver) = channel();
        let handle = Some(spawn_http_sender(receiver, dsn.clone()));
        Transport {
            sender: Mutex::new(sender),
            handle,
        }
    }

    /// Sends a sentry event and return the event ID.
    pub fn send_event(&self, mut event: Event) -> Uuid {
        if event.id.is_none() {
            event.id = Some(Uuid::new_v4());
        }
        let event_id = event.id.unwrap();
        // ignore the error on shutdown
        self.sender.lock().unwrap().send(Some(event)).ok();
        event_id
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        if let Ok(sender) = self.sender.lock() {
            sender.send(None).ok();
            if let Some(handle) = self.handle.take() {
                handle.join().ok();
            }
        }
    }
}
