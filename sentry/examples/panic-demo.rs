fn main() {
    let _sentry = sentry::init(sentry::ClientOptions::configure(|o| {
        o.set_release(sentry::release_name!())
    }));

    {
        let _guard = sentry::Hub::current().push_scope();
        sentry::configure_scope(|scope| {
            scope.set_tag("foo", "bar");
        });
        panic!("Holy shit everything is on fire!");
    }
}
