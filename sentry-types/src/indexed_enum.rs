//! Defines the [`IndexedEnum`] trait.

/// Trait for enums that have fixed indexed variants.
pub trait IndexedEnum: private::Sealed + Sized + 'static {
    /// A slice containing all the variants in index order.
    ///
    /// The exact value is subject to change in any future release.
    const VARIANTS: &[Self];

    /// Returns this variant's unique zero-based index.
    ///
    /// The index satisfies `0 <= self.as_index() < Self::VARIANT_COUNT`.
    fn as_index(&self) -> usize;
}

pub(crate) mod private {
    pub trait Sealed {}
}
