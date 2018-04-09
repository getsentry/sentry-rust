/// Statically returns the release name for sentry.
///
/// This can be used with `ClientOptions` to set the release name.
#[macro_export]
macro_rules! sentry_crate_release {
    () => {
        format!("{}@{}",
                module_path!().split("::").next().unwrap(),
                env!("CARGO_PKG_VERSION"))
    }
}
