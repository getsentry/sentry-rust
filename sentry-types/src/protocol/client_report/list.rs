//! Module with code for representing the underlying list of client reports.

use serde::{Deserialize, Serialize};

use super::{Category, Reason};
use crate::IndexedEnum as _;

/// The number of possible data-category/discard-reason combinations.
const POSSIBLE_CATEGORY_REASONS: usize = Category::VARIANT_COUNT * Reason::VARIANT_COUNT;

/// An entry in a client report.
///
/// Contains the quantity dropped for a certain category and reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    category: Category,
    reason: Reason,
    quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct ClientReportList(Vec<Item>);

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
                let &Item {
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

impl Item {
    /// Create a new [`ClientReportItem`].
    pub fn new(category: Category, reason: Reason, quantity: u64) -> Self {
        Self {
            category,
            reason,
            quantity,
        }
    }
}

impl FromIterator<Item> for ClientReportList {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Item>,
    {
        Self(iter.into_iter().collect())
    }
}

impl PartialEq for ClientReportList {
    fn eq(&self, other: &Self) -> bool {
        self.aggregate() == other.aggregate()
    }
}

fn aggregate_index(category: Category, reason: Reason) -> usize {
    category
        .as_index()
        .checked_mul(Reason::VARIANT_COUNT)
        .and_then(|product| product.checked_add(reason.as_index()))
        .expect("index should not overflow usize")
}
