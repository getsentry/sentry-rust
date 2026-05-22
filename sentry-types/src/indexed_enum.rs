//! Defines the [`IndexedEnum`] trait.

/// Trait for enums that have fixed indexed variants.
pub trait IndexedEnum: private::Sealed {
    /// The number of variants in this enum.
    const VARIANT_COUNT: usize;

    /// Returns this variant's unique zero-based index.
    ///
    /// The index satisfies `0 <= self.as_index() < Self::VARIANT_COUNT`.
    fn as_index(&self) -> usize;

    /// Returns an iterator over the enum variants in index order.
    fn iter_variants() -> impl Iterator<Item = Self>;
}

pub(crate) mod private {
    pub trait Sealed {}
}
