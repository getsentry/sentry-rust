use uuid::Uuid;

use api::Dsn;
use scope::Scope;
use protocol::Event;
use transport::Transport;
use errorlike::ErrorLike;

/// The sentry client object.
#[derive(Debug)]
pub struct Client {
    dsn: Option<Dsn>,
    transport: Transport,
}

impl Client {
    /// Creates a new sentry client for the given DSN.
    pub fn new(dsn: Dsn) -> Client {
        let transport = Transport::new(&dsn);
        Client {
            dsn: Some(dsn),
            transport: transport,
        }
    }

    fn prepare_event(&self, event: &mut Event, scope: Option<&Scope>) {
        if let Some(scope) = scope {
            if !scope.breadcrumbs.is_empty() {
                event
                    .breadcrumbs
                    .extend(scope.breadcrumbs.iter().map(|x| x.clone()));
            }
        }
    }

    /// Returns the DSN that constructed this client.
    pub fn dsn(&self) -> Option<&Dsn> {
        self.dsn.as_ref()
    }

    /// Captures an event and sends it to sentry.
    pub fn capture_event(&self, mut event: Event, scope: Option<&Scope>) -> Uuid {
        self.prepare_event(&mut event, scope);
        self.transport.send_event(event)
    }

    /// Captures an exception like thing.
    pub fn capture_exception<E: ErrorLike + ?Sized>(&self, e: &E, scope: Option<&Scope>) -> Uuid {
        self.capture_event(
            Event {
                exceptions: e.exceptions(),
                level: e.level(),
                ..Default::default()
            },
            scope,
        )
    }
}
