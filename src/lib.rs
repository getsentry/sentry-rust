//! This crate provides common types for working with the Sentry protocol or the
//! Sentry server.  It's used by the Sentry Relay infrastructure as well as the
//! rust Sentry client.
//!
//! Since this library is used in the Sentry relay as well it depends on
//! `serde_json` with the `preserve_order` feature.  As such all maps used
//! by the protocol are linked hash maps.
#![warn(missing_docs)]
extern crate chrono;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate url;
extern crate url_serde;
extern crate uuid;
extern crate linked_hash_map;

#[macro_use]
mod macros;

mod auth;
mod dsn;
mod project_id;
pub mod protocol;

pub use auth::*;
pub use dsn::*;
pub use project_id::*;
