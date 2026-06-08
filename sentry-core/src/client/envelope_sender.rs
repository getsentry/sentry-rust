//! Contains abstractions for sending envelopes.
//!
//! The most important type here is the [`EnvelopeSender`] struct, which wraps a [`Transport`] and
//! centralizes envelope sending logic. All code in this crate should send envelopes via the
//! [`EnvelopeSender`], not by using the [`Transport`] directly.

use std::sync::Arc;
use std::time::Duration;

use self::slot::TransportSlot;
use crate::{Envelope, Transport};

/// Sends envelopes through the client's transport.
///
/// Cloning this sender has `Arc`-like semantics: clones share the same transport
/// slot and send to the same underlying transport until it is shut down.
///
/// The [`Default`] implementation creates an [`EnvelopeSender`] without an underlying transport,
/// effectively rendering calls a no-op.
#[derive(Clone, Default)]
pub(crate) struct EnvelopeSender {
    transport_slot: TransportSlot<dyn Transport>,
}

impl EnvelopeSender {
    /// Sends an envelope if the transport is still available.
    pub(crate) fn send_envelope(&self, envelope: Envelope) {
        self.transport_slot.send_envelope(envelope);
    }

    /// Creates a sender from a shared transport slot.
    pub(super) fn new(transport: Arc<dyn Transport>) -> Self {
        let transport_slot = TransportSlot::new(transport);

        Self { transport_slot }
    }

    /// Flushes the transport if it is still available.
    pub(super) fn flush(&self, timeout: Duration) -> bool {
        self.transport_slot.flush(timeout)
    }

    /// Shuts down and removes the transport if it is still available.
    pub(super) fn shutdown(&self, timeout: Duration) -> bool {
        self.transport_slot.shutdown(timeout)
    }

    pub(super) fn clone_with_new_transport_slot(&self) -> Self {
        let transport_slot = self.transport_slot.clone_into_new_slot();
        Self { transport_slot }
    }

    /// Returns whether this sender currently has an available transport.
    pub(super) fn is_enabled(&self) -> bool {
        self.transport_slot.is_occupied()
    }
}

mod slot {
    use std::sync::{Arc, RwLock};
    use std::time::Duration;

    use sentry_types::protocol::v7::Envelope;

    use crate::Transport;

    const READ_EXPECT_MSG: &str = "could not acquire transport read lock";
    const WRITE_EXPECT_MSG: &str = "could not acquire transport write lock";

    /// A transport slot, which may or may not wrap a [`Transport`].
    ///
    /// When initially constructed with [`TransportSlot::new`], this type will be wrapping this
    /// transport. As long as constructed with a [`Transport`], as intended, this type also
    /// implements [`Transport`], and all of the method calls forward to the underlying transport.
    /// However, after [`Transport::shutdown`] is called on this slot, the slot is emptied, and
    /// all future [`Transport`] method calls become no-ops. The type provides
    /// [`Self::is_occupied`] to check if the transport is still present.
    ///
    /// This type has [`Arc`]-like clone semantics: clones share the underlying transport, and also
    /// share the slot occupied status.
    #[derive(Debug)]
    pub(super) struct TransportSlot<T: ?Sized> {
        inner: Arc<RwLock<Option<Arc<T>>>>,
    }

    impl<T: ?Sized> TransportSlot<T> {
        /// Create a new, occupied [`TransportSlot`] wrapping the provided transport.
        pub(super) fn new(transport: Arc<T>) -> Self {
            let inner = Arc::new(RwLock::new(Some(transport)));

            Self { inner }
        }

        /// Determine whether the slot is occupied, i.e. whether there is a transport inside.
        pub(super) fn is_occupied(&self) -> bool {
            self.inner.read().expect(READ_EXPECT_MSG).is_some()
        }

        /// Creates a new [`TransportSlot`] with the same underlying `Transport`, but in a new
        /// slot.
        ///
        /// If there is no transport, then we just return a clone of this empty slot. As empty
        /// slots cannot become occupied later, this has the same semantics as returning a new
        /// empty slot.
        pub(super) fn clone_into_new_slot(&self) -> Self {
            self.inner
                .read()
                .expect(READ_EXPECT_MSG)
                .as_ref()
                .map(|transport| Self::new(transport.clone()))
                .unwrap_or_else(|| self.clone())
        }
    }

    impl<T> Transport for TransportSlot<T>
    where
        T: Transport + ?Sized,
    {
        fn send_envelope(&self, envelope: Envelope) {
            if let Some(transport) = self.inner.read().expect(READ_EXPECT_MSG).as_deref() {
                transport.send_envelope(envelope);
            }
        }

        fn flush(&self, timeout: Duration) -> bool {
            self.inner
                .read()
                .expect(READ_EXPECT_MSG)
                .as_deref()
                .map(|transport| transport.flush(timeout))
                .unwrap_or(true)
        }

        fn shutdown(&self, timeout: Duration) -> bool {
            let transport_opt = self.inner.write().expect(WRITE_EXPECT_MSG).take();
            if let Some(transport) = transport_opt {
                sentry_debug!("client close; request transport to shut down");
                transport.shutdown(timeout)
            } else {
                sentry_debug!("client close; no transport to shut down");
                true
            }
        }
    }

    impl<T: ?Sized> Clone for TransportSlot<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }

    impl<T: ?Sized> Default for TransportSlot<T> {
        /// Creates an empty, no-op [`TransportSlot`].
        fn default() -> Self {
            Self {
                inner: Default::default(),
            }
        }
    }
}
