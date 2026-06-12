//! Contains the [`ClientReportRecorder`] type, which allows recording data losses.
//!
//! This type is `pub` to allow transports, which are defined outside the `sentry-core` crate, to
//! record lost events, without giving full access to the underlying [`ClientReportAggregator`].

use std::sync::{Arc, Weak};

use sentry_types::protocol::v7::client_report::{Category, Reason};

use super::{ClientReportAggregator, ClientReportAggregatorInner};

/// A handle for recording lost Sentry data.
///
/// Lost items recorded here will be aggregated into a [client report] and eventually sent to
/// Sentry. We attempt to send client reports with a future envelope, so recording lost events
/// should not lead to increased requests to Sentry.
///
/// Cloning has [`Arc`]-like semantics in the sense that clones record into the same client report
/// aggregator.
///
/// As client reports require atomics for aggregation, this struct's methods are no-ops on
/// platforms which lack support for 8-bit and/or 64-bit atomic operations.
///
/// [client report]: https://develop.sentry.dev/sdk/telemetry/client-reports/
#[derive(Debug, Clone)]
pub struct ClientReportRecorder {
    /// The inner aggregator.
    ///
    /// As the recorder only records losses, but cannot retrieve them, it does not make sense for
    /// the recorder to keep the underlying aggregator alive.
    ///
    /// We therefore store `inner` as a [`Weak`] so that we do not keep the aggregator alive.
    ///
    /// In practice, we would expect the recorder not to outlive the underlying aggregator, but in
    /// case it happens, it makes sense to make the `Weak` relationship explicit.
    #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
    inner: Weak<ClientReportAggregatorInner>,
}

impl ClientReportRecorder {
    /// Record `quantity` lost items, of the given `category`, discarded for the given `reason`.
    pub fn record_loss(&self, category: Category, reason: Reason, quantity: u64) {
        #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
        if let Some(aggregator) = self.aggregator() {
            aggregator.record_loss(category, reason, quantity);
        }
        #[cfg(not(all(target_has_atomic = "8", target_has_atomic = "64")))]
        let _ = (category, reason, quantity);
    }

    /// Creates a new no-op [`ClientReportRecorder`].
    ///
    /// This is used in backwards-compatibility code to handle the case where we might not have an
    /// aggregator.
    ///
    /// To get a useful [`ClientReportRecorder`], use [`ClientReportAggregator::recorder`].
    pub(crate) fn new_no_op() -> Self {
        Self {
            #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
            inner: Weak::new(),
        }
    }

    /// Create a new [`ClientReportRecorder`] which records into the given
    /// [`ClientReportAggregator`].
    pub(super) fn new(aggregator: &ClientReportAggregator) -> Self {
        #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
        {
            let ClientReportAggregator {
                inner: aggregator_inner,
            } = aggregator;

            let inner = Arc::downgrade(aggregator_inner);
            Self { inner }
        }
        #[cfg(not(all(target_has_atomic = "8", target_has_atomic = "64")))]
        {
            let _ = aggregator;
            Self {}
        }
    }

    /// Helper to obtain the [`ClientReportAggregator`] we record into, if still alive.
    ///
    /// This works by upgrading the [`Weak`] pointer to the [`ClientReportAggregatorInner`] stored
    /// in `self.inner`, then wrapping it in a [`ClientReportAggregator`].
    #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
    fn aggregator(&self) -> Option<ClientReportAggregator> {
        self.inner
            .upgrade()
            .map(|inner| ClientReportAggregator { inner })
    }
}
