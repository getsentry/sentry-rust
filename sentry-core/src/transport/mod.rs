use std::sync::Arc;
use std::time::Duration;

use crate::{ClientOptions, Envelope};

mod options;

pub use self::options::TransportOptions;

/// The trait for transports.
///
/// A transport is responsible for sending events to Sentry.  Custom implementations
/// can be created to use a different abstraction to send events.  This is for instance
/// used for the test system.
pub trait Transport: Send + Sync + 'static {
    /// Sends an [`Envelope`].
    ///
    /// [`Envelope`]: struct.Envelope.html
    fn send_envelope(&self, envelope: Envelope);

    /// Flushes the transport queue if there is one.
    ///
    /// If the queue was successfully drained, the return value should be
    /// `true` or `false` if events were left in it.
    fn flush(&self, timeout: Duration) -> bool {
        let _timeout = timeout;
        true
    }

    /// Instructs the Transport to shut down.
    fn shutdown(&self, timeout: Duration) -> bool {
        self.flush(timeout)
    }
}

/// A factory creating transport instances.
///
/// Because options are potentially reused between different clients the
/// [`ClientOptions`] do not actually contain a transport but a factory object that
/// can create transports instead.
///
/// This factory has two methods. Although both methods have default implementations, the default
/// implementations call each other, so to avoid an infinitely recursive loop, types implementing
/// this trait **must implement at least one of these methods**. We recommend that implementors
/// implement only [`TransportFactory::create_transport_with_options`] because the other method,
/// [`TransportFactory::create_transport`] only exists for backwards compatibility.
///
/// Both factory methods create a new transport wrapped in an [`Arc`]. Because transports can be
/// wrapped in `Arc`s and those are clonable, `Arc<Transport>` is also a valid transport factory.
/// This for instance lets you put a `Arc<TestTransport>` directly into the options.
pub trait TransportFactory: Send + Sync {
    /// Create a transport with the given `options`.
    ///
    /// Although a default implementation is provided for this trait method, we recommend that all
    /// custom transport factories implement this method, as it is the way the SDK constructs the
    /// transport. The default implementaton calls [`TransportFactory::create_transport`] as a
    /// fallback.
    fn create_transport_with_options(&self, options: TransportOptions) -> Arc<dyn Transport> {
        #[expect(deprecated, reason = "need to call deprecated method for back-compat")]
        self.create_transport(&options.into_client_options())
    }

    /// The legacy method for creating a transport.
    ///
    /// This method exists for backwards compatiblity with custom transport factories, which were
    /// created before [`TransportFactory::create_transport_with_options`] was added, and thus only
    /// implement this method.
    ///
    /// New custom transport factories **should not** implement this method, as it is not called
    /// from the SDK. A sensible default implementation, which forwards to
    /// [`TransportFactory::create_transport_with_options`] is provided.
    #[deprecated = "use and implement `create_transport_with_options` instead"]
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport> {
        TransportOptions::try_from_client_options(options).map_or_else(
            || {
                let no_op: Arc<dyn Transport> = Arc::new(NoOpTransport);
                no_op
            },
            |options| self.create_transport_with_options(options),
        )
    }
}

/// A no-op transport.
///
/// This is returned by [`TransportFactory::create_transport`] when called without a `dsn` in the
/// [`ClientOptions`], rendering the transport disabled.
struct NoOpTransport;

/// This implementor is **deprecated**, as the closure is used as the deprecated
/// [`TransportFactory::create_transport`] method.
///
/// Use or create a [`TransportFactory`] which provides
/// [`TransportFactory::create_transport_with_options`], instead.
impl<F> TransportFactory for F
where
    F: Fn(&ClientOptions) -> Arc<dyn Transport> + Clone + Send + Sync + 'static,
{
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport> {
        (*self)(options)
    }
}

impl<T: Transport> Transport for Arc<T> {
    fn send_envelope(&self, envelope: Envelope) {
        (**self).send_envelope(envelope)
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        (**self).shutdown(timeout)
    }
}

impl<T: Transport> TransportFactory for Arc<T> {
    fn create_transport_with_options(&self, _: TransportOptions) -> Arc<dyn Transport> {
        self.clone()
    }
}

impl Transport for NoOpTransport {
    fn send_envelope(&self, envelope: Envelope) {
        let _ = envelope;
    }
}
