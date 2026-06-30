//! Contains the [`ClientReportAggregatorInner`] type.
//!
//! Separate module as all this stuff requires atomics.

#![cfg(all(target_has_atomic = "64", target_has_atomic = "8"))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sentry_types::protocol::v7::client_report::{Category, Item, Reason, Report};
use sentry_types::IndexedEnum;

const ARRAY_SIZE: usize = Reason::VARIANTS.len() * Category::VARIANTS.len();

#[derive(Debug)]
pub(super) struct ClientReportAggregatorInner {
    inner: [AtomicU64; ARRAY_SIZE],
    has_reports: AtomicBool,
}

impl ClientReportAggregatorInner {
    pub(super) fn record_loss(&self, category: Category, reason: Reason, quantity: u64) {
        if quantity > 0 {
            self.inner[index(category, reason)].fetch_add(quantity, Ordering::Relaxed);
            self.has_reports.store(true, Ordering::Release);
        }
    }

    /// Aggregate the counts into a Client [`Report`], resetting them to zero.
    ///
    /// Only nonzero counts are included in the report.
    ///
    /// We only return a report if there is at least one dropped item to report. Otherwise, we
    /// return [`None`] to indicate that there is nothing to be sent.
    pub(super) fn take_pending_report(&self) -> Option<Report> {
        let reports = self.take_pending_report_vec();

        match reports.as_slice() {
            [] => None,
            [_, ..] => Some(Report::new(reports)),
        }
    }

    /// Aggregate the counts into a vector, resetting all counts in the aggregator to zero.
    ///
    /// Only nonzero quantities are included.
    fn take_pending_report_vec(&self) -> Vec<Item> {
        if !self.has_reports.swap(false, Ordering::Acquire) {
            return vec![];
        }

        iter_reason_categories()
            .zip(self.inner.iter())
            .map(|(cr, quantity)| (cr, quantity.swap(0, Ordering::Relaxed)))
            .filter(|&(_, quantity)| quantity > 0)
            .map(|(CategoryReason { category, reason }, quantity)| {
                Item::new(category, reason, quantity)
            })
            .collect()
    }
}

/// A category-reason pair in a Client Report.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CategoryReason {
    category: Category,
    reason: Reason,
}

/// Computes the index in the [`ClientReportAggregatorInner`]'s array for a category-reason pair.
fn index(category: Category, reason: Reason) -> usize {
    category
        .as_index()
        .checked_mul(Reason::VARIANTS.len())
        .and_then(|product| product.checked_add(reason.as_index()))
        .expect("should not overflow usize")
}

/// Iterates the category-reason pairs. The zero-indexed n-th item returned from this category
/// is the category-reason that the n-th item in the array corresponds to.
fn iter_reason_categories() -> impl Iterator<Item = CategoryReason> {
    Category::VARIANTS.iter().flat_map(|&category| {
        Reason::VARIANTS
            .iter()
            .map(move |&reason| CategoryReason { category, reason })
    })
}

impl Default for ClientReportAggregatorInner {
    fn default() -> Self {
        Self {
            inner: [const { AtomicU64::new(0) }; ARRAY_SIZE],
            has_reports: Default::default(),
        }
    }
}
