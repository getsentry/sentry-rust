//! Contains the [`ClientReportAggregatorInner`] type.
//!
//! Separate module as all this stuff requires atomics.

#![cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sentry_types::protocol::v7::{ClientReport, ClientReportItem, DataCategory, DiscardReason};
use sentry_types::IndexedEnum as _;

const ARRAY_SIZE: usize = DiscardReason::VARIANT_COUNT * DataCategory::VARIANT_COUNT;

#[derive(Debug, Default)]
pub(super) struct ClientReportAggregatorInner {
    inner: [AtomicU64; ARRAY_SIZE],
    has_reports: AtomicBool,
}

impl ClientReportAggregatorInner {
    pub(super) fn record_loss(&self, category: DataCategory, reason: DiscardReason, quantity: u64) {
        if quantity > 0 {
            self.inner[index(category, reason)].fetch_add(quantity, Ordering::Relaxed);
            self.has_reports.store(true, Ordering::Release);
        }
    }

    /// Aggregate the counts into a [`ClientReport`], resetting them to zero.
    ///
    /// Only nonzero counts are included in the report.
    ///
    /// We only return a report if there is at least one dropped item to report. Otherwise, we
    /// return [`None`] to indicate that there is nothing to be sent.
    pub(super) fn take_pending_report(&self) -> Option<ClientReport> {
        let reports = self.take_pending_report_vec();

        match reports.as_slice() {
            [] => None,
            [_, ..] => Some(ClientReport::new(reports)),
        }
    }

    /// Aggregate the counts into a vector, resetting all counts in the aggregator to zero.
    ///
    /// Only nonzero quantities are included.
    fn take_pending_report_vec(&self) -> Vec<ClientReportItem> {
        if !self.has_reports.swap(false, Ordering::Acquire) {
            return vec![];
        }

        iter_reason_categories()
            .zip(self.inner.iter())
            .map(|(cr, quantity)| (cr, quantity.swap(0, Ordering::Relaxed)))
            .filter(|&(_, quantity)| quantity > 0)
            .map(|(CategoryReason { category, reason }, quantity)| {
                ClientReportItem::new(category, reason, quantity)
            })
            .collect()
    }
}

/// A category-reason pair in a Client Report.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CategoryReason {
    category: DataCategory,
    reason: DiscardReason,
}

/// Computes the index in the [`ClientReportAggregatorInner`]'s array for a category-reason pair.
fn index(category: DataCategory, reason: DiscardReason) -> usize {
    category
        .as_index()
        .checked_mul(DiscardReason::VARIANT_COUNT)
        .and_then(|product| product.checked_add(reason.as_index()))
        .expect("should not overflow usize")
}

/// Iterates the category-reason pairs. The zero-indexed n-th item returned from this category
/// is the category-reason that the n-th item in the array corresponds to.
fn iter_reason_categories() -> impl Iterator<Item = CategoryReason> {
    DataCategory::iter_variants().flat_map(|category| {
        DiscardReason::iter_variants().map(move |reason| CategoryReason { category, reason })
    })
}
