use std::sync::Arc;
use std::time::Duration;

use crate::protocol::Event;
use crate::ClientOptions;

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

/// A factory creating transport instances.
///
/// Because options are potentially reused between different clients the
/// options do not actually contain a transport but a factory object that
/// can create transports instead.
///
/// The factory has a single method that creates a new arced transport.
/// Because transports can be wrapped in `Arc`s and those are clonable
/// any `Arc<Transport>` is also a valid transport factory.  This for
/// instance lets you put a `Arc<TestTransport>` directly into the options.
///
/// This is automatically implemented for all closures optionally taking
/// options and returning a boxed factory.
pub trait TransportFactory: Send + Sync {
    /// Given some options creates a transport.
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport>;
}

impl<F> TransportFactory for F
where
    F: Fn(&ClientOptions) -> Arc<dyn Transport> + Clone + Send + Sync + 'static,
{
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport> {
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
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport> {
        let _options = options;
        self.clone()
    }
}
