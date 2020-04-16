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

#![deny(missing_docs)]
// hm, this lint does not trigger correctly for some reason
#![warn(missing_doc_code_examples)]

mod api;
mod breadcrumbs;
mod client;
mod hub;
mod scope;

pub use api::*;
pub use breadcrumbs::IntoBreadcrumbs;
pub use client::Client;
pub use hub::Hub;
pub use scope::{Scope, ScopeGuard};

pub use sentry_types::protocol::v7 as protocol;
pub use sentry_types::protocol::v7::{Breadcrumb, Event, Level, User};
pub use sentry_types::Uuid;
