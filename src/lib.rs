extern crate backtrace;
extern crate futures;
extern crate reqwest;
extern crate sentry_types;
extern crate serde;
extern crate serde_json;
extern crate url;
extern crate uuid;

#[macro_use]
extern crate lazy_static;

#[cfg(feature = "with_failure")]
extern crate failure;

#[cfg(feature = "with_log")]
extern crate log;

// re-export common types from sentry types
pub use sentry_types::{Dsn, ProjectId};

// re-export the sentry protocol.
pub use sentry_types::protocol::v7 as protocol;

mod client;
mod constants;
mod transport;
mod scope;

pub use client::Client;
