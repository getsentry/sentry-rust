use std::fmt;
use std::sync::Arc;

use crate::Breadcrumb;

/// Type alias for before event/breadcrumb handlers.
pub type BeforeCallback<T> = Arc<dyn Fn(T) -> Option<T> + Send + Sync>;

/// TODO
#[derive(Clone)]
pub struct ClientOptions {
    pub(crate) max_breadcrumbs: usize,
    pub(crate) before_breadcrumb: Option<BeforeCallback<Breadcrumb>>,
}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct BeforeBreadcrumbSet(bool);
        f.debug_struct("ClientOptions")
            .field("max_breadcrumbs", &self.max_breadcrumbs)
            .field(
                "before_breadcrumb",
                &BeforeBreadcrumbSet(self.before_breadcrumb.is_some()),
            )
            .finish()
    }
}
