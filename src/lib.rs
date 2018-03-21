//! This crate provides common types for working with the Sentry protocol or the
//! Sentry server.  It's used by the sentry relay infrastructure as well as the
//! rust Sentry client/.
#![warn(missing_docs)]
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde_plain;
extern crate url;
extern crate url_serde;

#[macro_use]
mod macros;

mod auth;
mod dsn;
mod project_id;
pub mod protocol;

pub use auth::*;
pub use dsn::*;
pub use project_id::*;
