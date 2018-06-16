#[cfg(feature = "with_client_implementation")]
mod real;

#[cfg(not(feature = "with_client_implementation"))]
pub(crate) mod noop;

#[cfg(feature = "with_client_implementation")]
pub use self::real::*;

#[cfg(not(feature = "with_client_implementation"))]
pub use self::noop::*;
