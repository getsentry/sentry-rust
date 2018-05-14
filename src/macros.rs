/// Returns the intended release for Sentry as an `Option<Cow<'static, str>>`.
///
/// This can be used with `ClientOptions` to set the release name.  It uses
/// the information supplied by cargo to calculate a release.
#[macro_export]
#[cfg(feature = "with_client_implementation")]
macro_rules! sentry_crate_release {
    () => {{
        use std::sync::{Once, ONCE_INIT};
        static mut INIT: Once = ONCE_INIT;
        static mut RELEASE: Option<String> = None;
        unsafe {
            INIT.call_once(|| {
                RELEASE = option_env!("CARGO_PKG_NAME").and_then(|name| {
                    option_env!("CARGO_PKG_VERSION").map(|version| format!("{}@{}", name, version))
                });
            });
            RELEASE.as_ref().map(|x| {
                let release: &'static str = ::std::mem::transmute(x.as_str());
                ::std::borrow::Cow::Borrowed(release)
            })
        }
    }};
}

macro_rules! with_client_impl {
    ($body:block) => {
        #[cfg(feature = "with_client_implementation")]
        {
            $body
        }
        #[cfg(not(feature = "with_client_implementation"))]
        {
            Default::default()
        }
    };
}

#[allow(unused_macros)]
macro_rules! shim_unreachable {
    () => {
        panic!(
            "this code should not be reachable. It's stubbed out for shim usage. \
             If you get this error this is a bug in the sentry shim"
        );
    };
}
