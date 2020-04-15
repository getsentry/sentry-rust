//! <p style="margin: -10px 0 0 15px; padding: 0; float: right;">
//!   <a href="https://sentry.io/"><img
//!     src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png"
//!     style="width: 260px"></a>
//! </p>
//!
//! This crate provides support for logging events and errors to the
//! [Sentry](https://sentry.io/) error logging service.
//! It represents the core of sentry and provides APIs for instrumenting code,
//! and to write integrations that can generate events or hook into the event
//! processing pipeline.

pub use sentry_types::protocol::v7 as protocol;
pub use sentry_types::protocol::v7::{Breadcrumb, Level, User};
