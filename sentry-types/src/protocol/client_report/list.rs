//! Module with code for representing the underlying list of client reports.

use serde::{Deserialize, Serialize};

use super::{DataCategory, DiscardReason};
use crate::IndexedEnum as _;

/// The number of possible data-category/discard-reason combinations.
const POSSIBLE_CATEGORY_REASONS: usize = DataCategory::VARIANT_COUNT * DiscardReason::VARIANT_COUNT;

/// An entry in a client report.
///
/// Contains the quantity dropped for a certain category and reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientReportItem {
    category: DataCategory,
    reason: DiscardReason,
    quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ClientReportList(Vec<ClientReportItem>);

impl ClientReportList {
    /// Private helper used in [`PartialEq`] implementation to make comparisions order-insensitive.
    ///
    /// This function aggregates all the counts into an array, where each item in the array
    /// represents the counts for a given category-reason pair. If the value returned by this
    /// function is identical for two separate `ClientReportList`s, then these should be
    /// considered equal to each other.
    fn aggregate(&self) -> [u64; POSSIBLE_CATEGORY_REASONS] {
        self.0
            .iter()
            .map(|item| {
                let &ClientReportItem {
                    category,
                    reason,
                    quantity,
                } = item;
                (aggregate_index(category, reason), quantity)
            })
            .fold(
                [0; POSSIBLE_CATEGORY_REASONS],
                |mut counts, (index, quantity)| {
                    let count = counts.get_mut(index).expect("index must be in bounds");
                    *count = count.saturating_add(quantity);
                    counts
                },
            )
    }
}

impl ClientReportItem {
    /// Create a new [`ClientReportItem`].
    pub fn new(category: DataCategory, reason: DiscardReason, quantity: u64) -> Self {
        Self {
            category,
            reason,
            quantity,
        }
    }
}

impl FromIterator<ClientReportItem> for ClientReportList {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = ClientReportItem>,
    {
        Self(iter.into_iter().collect())
    }
}

impl PartialEq for ClientReportList {
    fn eq(&self, other: &Self) -> bool {
        self.aggregate() == other.aggregate()
    }
}

fn aggregate_index(category: DataCategory, reason: DiscardReason) -> usize {
    category
        .as_index()
        .checked_mul(DiscardReason::VARIANT_COUNT)
        .and_then(|product| product.checked_add(reason.as_index()))
        .expect("index should not overflow usize")
}
