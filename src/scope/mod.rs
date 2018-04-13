#[cfg(feature = "with_client_implementation")]
mod real;

#[cfg(any(not(feature = "with_client_implementation"), feature = "with_shim_api"))]
pub(crate) mod noop;

#[cfg(feature = "with_client_implementation")]
pub use self::real::*;

#[cfg(not(feature = "with_client_implementation"))]
pub use self::noop::*;

#[cfg(feature = "with_shim_api")]
pub mod shim {
    pub use super::noop::*;
}
