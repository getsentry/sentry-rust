#[macro_use]
extern crate log;
extern crate pretty_env_logger;
#[macro_use]
extern crate sentry;

use std::thread;

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry_crate_release!(),
            ..Default::default()
        },
    ));

    let mut log_builder = pretty_env_logger::formatted_builder().unwrap();
    log_builder.parse("info");
    sentry::integrations::log::init(Some(Box::new(log_builder.build())), Default::default());
    sentry::integrations::panic::register_panic_handler();

    info!("Spawning thread");

    thread::spawn(|| {
        info!("Spawned thread, configuring scope.");
        // configure the current thread's scope
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        // get the current scope's token so it can be propagated into a new thread.
        info!("Creating scope token.");
        let scope_handle = sentry::scope_handle();

        thread::spawn(|| {
            info!("Activating scope token in new thread.");
            // activates the scope token which binds the current context to the token's context.
            scope_handle.bind();
            error!("Failing!");
        }).join()
            .unwrap();
    }).join()
        .unwrap();
}
