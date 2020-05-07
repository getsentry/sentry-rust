fn main() {
    let _sentry = sentry::init("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");

    sentry::with_scope(
        |scope| {
            scope.set_level(Some(sentry::Level::Warning));
            scope.set_fingerprint(Some(["a-message"].as_ref()));
            scope.set_tag("foo", "bar");
        },
        || {
            panic!("Shit's on fire yo. 🔥 🚒");
        },
    );
}
