extern crate sentry;

fn main() {
    let _sentry = sentry::init("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");
    sentry::configure_scope(|scope| {
        scope.set_fingerprint(Some(["a-message"].as_ref()));
        scope.set_tag("foo", "bar");
    });

    sentry::capture_message("This is recorded as a warning now", sentry::Level::Warning);
}
