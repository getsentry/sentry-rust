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
        static mut RELEASE: Option<&'static str> = None;
        unsafe {
            INIT.call_once(|| {
                RELEASE = option_env!("CARGO_PKG_NAME").and_then(|name| {
                    option_env!("CARGO_PKG_VERSION").map(|version| format!("{}@{}", name, version))
                }).map(|x| Box::leak(x.into_boxed_str()) as &'static str)
            });
            RELEASE.map(::std::borrow::Cow::Borrowed)
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
macro_rules! minimal_unreachable {
    () => {
        panic!(
            "this code should not be reachable. It's stubbed out for minimal usage. \
             If you get this error this is a bug in the sentry minimal support"
        );
    };
}
