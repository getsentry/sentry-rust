use uuid::Uuid;

use Dsn;
use protocol::Event;
use transport::Transport;

pub struct Client {
    dsn: Option<Dsn>,
    transport: Option<Transport>,
}

impl Client {
    pub fn new(dsn: Option<Dsn>) -> Client {
        let transport = dsn.as_ref().map(Transport::new);
        Client { dsn, transport }
    }

    pub fn dsn(&self) -> Option<&Dsn> {
        self.dsn.as_ref()
    }

    pub fn capture_event(&self, event: Event) -> Uuid {
        self.transport
            .as_ref()
            .map(|transport| transport.send_event(event))
            .unwrap_or(Uuid::nil())
    }
}
