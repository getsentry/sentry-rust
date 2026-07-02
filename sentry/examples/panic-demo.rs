fn main() {
    let _sentry = sentry::init(
        sentry::ClientOptions::new()
            .maybe_release(sentry::release_name!())
            .debug(true),
    );

    {
        let _guard = sentry::Hub::current().push_scope();
        sentry::configure_scope(|scope| {
            scope.set_tag("foo", "bar");
        });
        panic!("Holy shit everything is on fire!");
    }
}
