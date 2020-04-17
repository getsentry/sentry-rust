//! This module exposes the types for the Sentry protocol in different versions.

#[cfg(feature = "with_protocol")]
pub mod v7;

/// The latest version of the protocol.
pub const LATEST: u16 = 7;

/// the always latest sentry protocol version
#[cfg(feature = "with_protocol")]
pub mod latest {
    pub use super::v7::*;
}
