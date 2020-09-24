//! The Sentry Debug Images Integration.
//!
//! The [`DebugImagesIntegration`] adds metadata about the loaded shared
//! libraries to Sentry [`Event`]s.
//!
//! This Integration only works on Unix-like OSs right now. Support for Windows
//! will be added in the future.
//!
//! # Configuration
//!
//! The integration by default attaches this information to all [`Event`]s, but
//! a custom filter can be defined as well.
//!
//! ```
//! use sentry_core::Level;
//! let integration = sentry_debug_images::DebugImagesIntegration::new()
//!     .filter(|event| event.level >= Level::Warning);
//! ```
//!
//! [`Event`]: sentry_core::Event

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
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
