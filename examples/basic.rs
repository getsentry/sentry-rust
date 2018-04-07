extern crate failure;
extern crate sentry;

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: Some("16ebee932f262d6457d8713defc49714159c0a1a".into()),
            ..Default::default()
        },
    ));
    sentry::integrations::panic::register_panic_handler();

    let _scope_guard = sentry::push_and_configure_scope(|scope| {
        scope.set_tag("foo", "bar");
    });

    panic!("Holy shit everything is on fire!");
}
