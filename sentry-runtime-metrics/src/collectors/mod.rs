//! Built-in metric collectors.
//!
//! This module contains collectors for common runtime metrics:
//! - Memory usage
//! - Process statistics
//! - Async runtime metrics (Tokio)

mod memory;
mod process;

#[cfg(feature = "tokio-runtime")]
mod tokio_runtime;

pub use memory::MemoryCollector;
pub use process::ProcessCollector;

#[cfg(feature = "tokio-runtime")]
pub use tokio_runtime::TokioCollector;
