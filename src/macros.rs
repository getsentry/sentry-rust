/// Returns the intended release for Sentry as an option.
///
/// This can be used with `ClientOptions` to set the release name.  It uses
/// the information supplied by cargo to calculate a release.
#[macro_export]
macro_rules! sentry_crate_release {
    () => {
        option_env!("CARGO_PKG_NAME").and_then(|name| {
            option_env!("CARGO_PKG_VERSION")
                .map(|version| format!("{}@{}", name, version))
        })
    }
}
