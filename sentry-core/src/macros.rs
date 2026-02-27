/// Returns the intended release for Sentry as an `Option<Cow<'static, str>>`.
///
/// This can be used with `ClientOptions` to set the release name.  It uses
/// the information supplied by cargo to calculate a release.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate sentry;
/// # fn main() {
/// let _sentry = sentry::init(sentry::ClientOptions {
///     release: sentry::release_name!(),
///     ..Default::default()
/// });
/// # }
/// ```
#[macro_export]
macro_rules! release_name {
    () => {{
        use std::sync::Once;
        static mut INIT: Once = Once::new();
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

// TODO: temporarily exported for use in `sentry` crate
#[macro_export]
#[doc(hidden)]
macro_rules! with_client_impl {
    ($body:block) => {
        #[cfg(feature = "client")]
        {
            $body
        }
        #[cfg(not(feature = "client"))]
        {
            Default::default()
        }
    };
}

// TODO: temporarily exported for use in `sentry` crate
#[macro_export]
#[doc(hidden)]
macro_rules! sentry_debug {
    ($($arg:tt)*) => {
        $crate::Hub::with(|hub| {
            if hub.client().map_or(false, |c| c.options().debug) {
                eprint!("[sentry] ");
                eprintln!($($arg)*);
            }
        });
    }
}

/// Panics in debug builds and logs through `sentry_debug!` in non-debug builds.
#[macro_export]
#[doc(hidden)]
macro_rules! debug_panic_or_log {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        panic!($($arg)*);

        #[cfg(not(debug_assertions))]
        $crate::sentry_debug!($($arg)*);
    }};
}

/// If the condition is false, panics in debug builds and logs in non-debug builds.
#[macro_export]
#[doc(hidden)]
macro_rules! debug_assert_or_log {
    ($cond:expr $(,)?) => {{
        let condition = $cond;
        if !condition {
            $crate::debug_panic_or_log!("assertion failed: {}", stringify!($cond));
        }
    }};
    ($cond:expr, $($arg:tt)+) => {{
        let condition = $cond;
        if !condition {
            $crate::debug_panic_or_log!($($arg)+);
        }
    }};
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

#[cfg(test)]
mod tests {
    #[test]
    fn debug_assert_or_log_does_not_panic_when_condition_holds() {
        crate::debug_assert_or_log!(2 + 2 == 4, "should not panic");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "assertion failed: 1 == 2")]
    fn debug_assert_or_log_panics_with_default_message_when_condition_fails() {
        crate::debug_assert_or_log!(1 == 2);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "custom invariant message")]
    fn debug_assert_or_log_panics_with_custom_message_when_condition_fails() {
        crate::debug_assert_or_log!(false, "custom invariant message");
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn no_panic_without_debug_assertions() {
        crate::debug_assert_or_log!(false, "should not panic");
    }
}
