#![cfg(feature = "client")]

//! A module containing code for aggregating client reports.
//!
//! This module is a no-op on platforms lacking support for the atomics needed to collect client
//! reports.

#[cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]
use std::sync::Arc;

#[cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]
use sentry_types::protocol::v7::client_report::ItemLoss;
use sentry_types::protocol::v7::client_report::{Category, LossSource, Reason};
use sentry_types::protocol::v7::ClientReport;

#[cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]
use self::inner::ClientReportAggregatorInner;

mod inner;
mod recorder;

pub use self::recorder::{Recorder, TransportLossReason};

/// Aggregates counts for lost data that should be reported in client reports.
///
/// The aggregator records losses by [`Category`] and [`Reason`]. Recording a loss only
/// increments counters; no envelope is created at record time. Callers that are about to send an
/// envelope can call [`Self::take_pending_report`] to drain the current counters into a
/// [`ClientReport`] item and attach the report to the outgoing envelope.
///
/// Draining resets the counters that are included in the returned report. If no losses were
/// recorded since the previous drain, [`Self::take_pending_report`] returns [`None`].
///
/// This type is backed by an [`Arc`]. Cloning the aggregator has the same semantics as cloning an
/// [`Arc`]: clones share the same counters, and a drain from any clone resets the shared counters
/// for all clones.
///
/// This type is a no-op on platforms lacking support for the atomics needed to collect client
/// reports.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClientReportAggregator {
    #[cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]
    inner: Arc<ClientReportAggregatorInner>,
}

impl ClientReportAggregator {
    /// Create a new client report, with all zero counts.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Record lost Sentry data.
    ///
    /// Records the given Sentry telemetry item as discarded for the provided `reason`.
    pub(crate) fn record_lost_data<L: LossSource>(&self, data: &L, reason: Reason) {
        #[cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]
        data.losses().for_each(|loss| {
            let ItemLoss {
                category, quantity, ..
            } = loss;
            self.record_loss(category, reason, quantity)
        });
        #[cfg(not(all(target_has_atomic = "64", target_has_atomic = "8")))]
        let _ = (data, reason);
    }

    /// Records `quantity` lost items for `category` and `reason`.
    ///
    /// This method updates aggregate counters only. The loss is not sent until a later call to
    /// [`Self::take_pending_report`] drains the counters and returns a [`ClientReport`] for an
    /// outgoing envelope. A `quantity` of zero is ignored.
    pub(crate) fn record_loss(&self, category: Category, reason: Reason, quantity: u64) {
        #[cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]
        self.inner.record_loss(category, reason, quantity);

        #[cfg(not(all(target_has_atomic = "64", target_has_atomic = "8")))]
        let _ = (category, reason, quantity);
    }

    /// Drains recorded losses into a [`ClientReport`].
    ///
    /// The returned report contains only nonzero counters. Counters included in the report are reset
    /// before this method returns. If there are no recorded losses to report, this method returns
    /// [`None`].
    pub(crate) fn take_pending_report(&self) -> Option<ClientReport> {
        #[cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]
        {
            self.inner.take_pending_report()
        }

        #[cfg(not(all(target_has_atomic = "64", target_has_atomic = "8")))]
        {
            None
        }
    }

    /// Creates a [`Recorder`] which records into this aggregator.
    pub(super) fn recorder(&self) -> Recorder {
        Recorder::new(self)
    }
}
