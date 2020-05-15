//! The Sentry Debug Images Integration.
//!
//! The `DebugImagesIntegration` adds metadata about the loaded shared libraries
//! to Sentry `Event`s.
//!
//! # Configuration
//!
//! The integration by default attaches this information to all Events, but a
//! custom filter can be defined as well.
//!
//! ```
//! use sentry_core::Level;
//! let integration = sentry_debug_images::DebugImagesIntegration {
//!     filter: Box::new(|event| event.level >= Level::Warning),
//!     ..Default::default()
//! };
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]

#[cfg(unix)]
mod unix;

#[cfg(unix)]
use unix::debug_images;

#[cfg(not(unix))]
fn debug_images() -> Vec<sentry_core::protocol::DebugImage> {
    vec![]
}

mod integration;

pub use integration::DebugImagesIntegration;
