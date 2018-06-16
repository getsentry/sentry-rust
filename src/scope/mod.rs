#[cfg(feature = "with_client_implementation")]
mod real;

#[cfg(any(not(feature = "with_client_implementation"), feature = "with_minimal_api"))]
pub(crate) mod noop;

#[cfg(feature = "with_client_implementation")]
pub use self::real::*;

#[cfg(not(feature = "with_client_implementation"))]
pub use self::noop::*;

#[cfg(feature = "with_minimal_api")]
pub mod minimal {
    pub use super::noop::*;
}
