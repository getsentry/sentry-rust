#[warn(missing_docs)]
extern crate backtrace;
extern crate futures;
#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate sentry_types;
extern crate serde;
extern crate serde_json;
extern crate url;
extern crate uuid;

#[cfg(feature = "with_failure")]
extern crate regex;

#[cfg(feature = "with_failure")]
extern crate failure;

#[cfg(feature = "with_log")]
extern crate log;

mod client;
mod constants;
mod transport;
mod scope;
mod errorlike;
mod api;

pub use api::*;
