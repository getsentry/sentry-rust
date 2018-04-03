use std::sync::mpsc::{channel, Sender, Receiver};

use uuid::Uuid;
use reqwest::Client;
use protocol::Event;


/// A transport can send rust events.
pub struct Transport {
    client: Client,
    sender: Sender<Option<Event>>,
}

fn spawn_http_sender(receiver: Receiver<Option<Event>>) {
}

impl Transport {
    /// Creates a new client.
    pub fn new() -> Transport {
        let (sender, receiver) = channel();
        spawn_http_sender(receiver);
        Transport {
            client: Client::new(),
            sender: sender,
        }
    }

    /// Sends a sentry event and return the event ID.
    pub fn send_event(mut event: Event) -> Uuid {
        if event.id.is_none() {
            event.id = Some(Uuid::new_v4());
        }
        let event_id = event.id.unwrap();
        // ignore the error on shutdown
        self.sender.send(event).ok();
        event_id
    }
}
