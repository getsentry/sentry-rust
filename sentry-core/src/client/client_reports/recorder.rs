//! Contains the [`Recorder`] type, which allows recording data losses.
//!
//! This type is `pub` to allow transports, which are defined outside the `sentry-core` crate, to
//! record lost events, without giving full access to the underlying [`ClientReportAggregator`].

#[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
use std::sync::{Arc, Weak};

#[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
use sentry_types::protocol::v7::client_report::Reason;
use sentry_types::protocol::v7::EnvelopeItem;

use super::ClientReportAggregator;
#[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
use super::ClientReportAggregatorInner;

/// A handle for a transport to record lost Sentry data.
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
pub struct Recorder {
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

/// Reasons for which a transport might drop data.
///
/// This is a subset of [`Reason`], as defined in [`sentry_types`] because only some of those
/// reasons may be applicable to transports.
#[non_exhaustive]
pub enum TransportLossReason {}

impl Recorder {
    /// Record an envelope item lost for a given reason.
    pub fn record_lost_envelope_item(
        &self,
        envelope_item: &EnvelopeItem,
        reason: TransportLossReason,
    ) {
        #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
        if let Some(aggregator) = self.aggregator() {
            aggregator.record_lost_envelope_item(envelope_item, reason.into_reason());
        }
        #[cfg(not(all(target_has_atomic = "8", target_has_atomic = "64")))]
        let _ = (envelope, reason);
    }

    /// Creates a new no-op [`Recorder`].
    ///
    /// This is used in backwards-compatibility code to handle the case where we might not have an
    /// aggregator.
    ///
    /// To get a useful [`Recorder`], use [`ClientReportAggregator::recorder`].
    pub(crate) fn new_no_op() -> Self {
        Self {
            #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
            inner: Weak::new(),
        }
    }

    /// Create a new [`Recorder`] which records into the given
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

impl TransportLossReason {
    /// Convert to the corresponding [`Reason`].
    #[cfg(all(target_has_atomic = "8", target_has_atomic = "64"))]
    fn into_reason(self) -> Reason {
        match self {}
    }
}
