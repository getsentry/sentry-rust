//! This module provides support for various integrations.
//!
//! Which integerations are available depends on the features that were compiled in.
#[cfg(feature = "with_failure")]
pub mod failure;

#[cfg(feature = "with_error_chain")]
pub mod error_chain;

#[cfg(feature = "with_log")]
pub mod log;

#[cfg(feature = "with_env_logger")]
pub mod env_logger;

#[cfg(feature = "with_slog")]
pub mod slog;

#[cfg(feature = "with_panic")]
pub mod panic;
