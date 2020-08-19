//! Adds Contexts to Sentry Events
//!
//! This integration is enabled by default in `sentry` and adds `device`, `os`
//! and `rust` contexts to Events, as well as sets a `server_name` if not
//! already defined.
//!
//! See the [Contexts Interface] documentation for more info.
//!
//! # Examples
//!
//! ```
//! let integration = sentry_contexts::ContextIntegration {
//!     add_os: false,
//!     ..Default::default()
//! };
//! let _sentry = sentry::init(sentry::ClientOptions::default().add_integration(integration));
//! ```
//!
//! [Contexts Interface]: https://develop.sentry.dev/sdk/event-payloads/contexts/

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

mod integration;
mod utils;

pub use integration::ContextIntegration;
