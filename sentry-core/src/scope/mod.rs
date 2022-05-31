use sentry_types::protocol::v7::{Breadcrumb, User, Value};
use serde::{Deserialize, Serialize};

#[cfg(feature = "client")]
mod real;

#[cfg(not(feature = "client"))]
pub(crate) mod noop;

#[cfg(feature = "client")]
pub use self::real::*;

#[cfg(not(feature = "client"))]
pub use self::noop::*;

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ScopeUpdate {
    AddBreadcrumb(Breadcrumb),
    ClearBreadcrumbs,
    User(Option<User>),
    SetExtra(String, Value),
    RemoveExtra(String),
    SetTag(String, String),
    RemoveTag(String),
}
