//! This module provides support for various integrations.
//!
//! Which integerations are available depends on the features that were compiled in.
use std::any::TypeId;
#[cfg(feature = "with_client_implementation")]
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

#[cfg(feature = "with_client_implementation")]
use std::sync::Mutex;

#[cfg(feature = "with_client_implementation")]
use client::ClientOptions;

#[cfg(feature = "with_failure")]
pub mod failure;

#[cfg(feature = "with_error_chain")]
pub mod error_chain;

#[cfg(all(feature = "with_log", feature = "with_client_implementation"))]
pub mod log;

#[cfg(feature = "with_panic")]
pub mod panic;

#[cfg(feature = "with_client_implementation")]
lazy_static! {
    static ref ACTIVE_INTEGRATIONS: Mutex<Vec<TypeId>> = Mutex::new(Vec::new());
}

/// A reference to an integration of a specific type.
///
/// This is returned by `get_integration` on a hub to look up a configured
/// integration.  It derefs into the actual integration.
pub struct IntegrationRef<I> {
    pub(crate) integration: Arc<Box<Integration>>,
    pub(crate) _marker: PhantomData<I>,
}

impl<I: Integration> Deref for IntegrationRef<I> {
    type Target = I;

    fn deref(&self) -> &I {
        if self.integration.__get_internal_id__() == TypeId::of::<I>() {
            unsafe { &*(&**self.integration as *const Integration as *const I) }
        } else {
            panic!("typeid mismatch in integration ref");
        }
    }
}

/// Integration abstraction.
pub trait Integration: Debug + Sync + Send + 'static {
    /// Called whenever the integration is bound on a hub.
    #[cfg(feature = "with_client_implementation")]
    fn setup(&self, options: &ClientOptions) {
        let _options = options;
    }

    /// Called to initialize the integration.
    ///
    /// If the integration has been enabled before this method is not called.
    /// Because of this accessing data on `self` is not a good idea.
    fn setup_once(&self) {}

    /// Returns the internal ID of the integration.
    #[doc(hidden)]
    fn __get_internal_id__(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

#[cfg(feature = "with_client_implementation")]
fn setup_integration(
    integration: Arc<Box<Integration>>,
    options: &ClientOptions,
    rv: &mut HashMap<TypeId, Arc<Box<Integration>>>,
) {
    let mut active = ACTIVE_INTEGRATIONS.lock().unwrap();
    let id = integration.__get_internal_id__();
    if !active.contains(&id) {
        active.push(id);
        integration.setup_once();
        integration.setup(options);
    }
    rv.insert(id, integration.clone());
}

#[cfg(feature = "with_client_implementation")]
pub(crate) fn setup_integrations(
    options: &ClientOptions,
) -> HashMap<TypeId, Arc<Box<Integration>>> {
    let mut rv = HashMap::new();

    for integration in &options.integrations {
        setup_integration(integration.clone(), options, &mut rv);
    }

    if options.default_integrations {
        #[cfg(feature = "with_panic")]
        {
            use self::panic::PanicIntegration;
            if !rv.contains_key(&PanicIntegration.__get_internal_id__()) {
                setup_integration(
                    Arc::new(Box::new(PanicIntegration) as Box<Integration>),
                    options,
                    &mut rv,
                );
            }
        }
    }

    rv
}
