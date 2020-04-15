use sentry_types::Uuid;
use std::fmt;

use crate::{Breadcrumb, Event, Scope};

pub trait Client: fmt::Debug + Sync + Send + 'static {
    fn capture_event(&self, event: Event<'static>, scope: Option<&Scope>) -> Uuid;

    fn before_breadcrumb(&self, breadcrumb: Breadcrumb) -> Option<Breadcrumb>;
    fn max_breadcrumbs(&self) -> usize;
    fn send_default_pii(&self) -> bool;
    fn debug(&self) -> bool;
}
