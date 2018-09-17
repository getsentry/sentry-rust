//! This crate provides common types for working with the Sentry protocol or the
//! Sentry server.  It's used by the Sentry Relay infrastructure as well as the
//! rust Sentry client.
//!
//! Since this library is used in the Sentry relay as well it depends on
//! `serde_json` with the `preserve_order` feature.  As such all maps used
//! by the protocol are linked hash maps.
//!
//! Most of the types in this crate are serializable in one form or another.
//! The types in the `protocol` module are generally really only serializable
//! to JSON as other formats are not supported by Sentry at this date.
//!
//! ## Contents
//!
//! The crate provides a bunch of common types for working with Sentry as
//! such (DSN, ProjectIDs, authentication headers) as well as types for
//! the Sentry event protocol.
//!
//! Right now only `v7` of the protocol is implemented but it's versioned
//! so later versions might be added later.
//!
//! ## API Concepts
//!
//! Most types are directly serializable or deserializable and try to implement
//! the `Default` type.  This means that objects can be created conviently
//! and missing attributes can be filled in:
//!
//! ```rust
//! use sentry_types::protocol::v7;
//!
//! let event = v7::Event {
//!     message: Some("Hello World!".to_string()),
//!     culprit: Some("foo in bar".to_string()),
//!     level: v7::Level::Info,
//!     ..Default::default()
//! };
//! ```
#![warn(missing_docs)]
#![cfg_attr(feature = "cargo-clippy", allow(cast_lossless))]
#![cfg_attr(feature = "cargo-clippy", allow(trivially_copy_pass_by_ref))]

#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate serde_derive;

extern crate chrono;
extern crate debugid;
extern crate failure;
#[cfg(feature = "with_serde")]
extern crate serde;
#[cfg(feature = "with_serde")]
#[cfg(feature = "with_serde")]
extern crate serde_json;
extern crate url;
#[cfg(feature = "with_serde")]
extern crate url_serde;
extern crate uuid;

#[macro_use]
mod macros;

mod auth;
mod dsn;
mod project_id;
pub mod protocol;
mod utils;

pub use auth::*;
pub use dsn::*;
pub use project_id::*;

// Re-export external types and traits for convenience
pub use chrono::{DateTime, ParseError as ChronoParseError, TimeZone, Utc};
pub use debugid::*;
pub use uuid::{Uuid, Variant as UuidVariant, Version as UuidVersion};
