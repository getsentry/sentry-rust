use uuid::Uuid;

use Dsn;
use protocol::Event;
use transport::Transport;

/// The sentry client object.
pub struct Client {
    dsn: Option<Dsn>,
    transport: Option<Transport>,
}

impl Client {
    /// Creates a new sentry client for the given DSN.
    pub fn new(dsn: Dsn) -> Client {
        let transport = Transport::new(&dsn);
        Client {
            dsn: Some(dsn),
            transport: Some(transport),
        }
    }

    /// Creates a new disabled client.
    pub fn disabled() -> Client {
        Client {
            dsn: None,
            transport: None,
        }
    }

    /// Returns the DSN that constructed this client.
    pub fn dsn(&self) -> Option<&Dsn> {
        self.dsn.as_ref()
    }

    /// Captures an event and sends it to sentry.
    pub fn capture_event(&self, event: Event) -> Uuid {
        self.transport
            .as_ref()
            .map(|transport| transport.send_event(event))
            .unwrap_or(Uuid::nil())
    }
}
