use std::sync::Arc;
use std::thread;

fn main() {
    // this initializes sentry.  It also gives the thread that calls this some
    // special behavior in that all other threads spawned will get a hub based on
    // the hub from here.
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let mut log_builder = pretty_env_logger::formatted_builder();
    log_builder.parse("info");
    sentry::integrations::log::init(Some(Box::new(log_builder.build())), Default::default());
    sentry::integrations::panic::register_panic_handler();

    // the log integration sends to Hub::current()
    log::info!("Spawning thread");

    thread::spawn(|| {
        // The thread spawned here gets a new hub cloned from the hub of the
        // main thread.
        log::info!("Spawned thread, configuring scope.");

        // now we want to create a new hub based on the thread's normal hub for
        // working with it explicitly.
        let hub = Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));

        // reconfigure that scope.
        hub.configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        // we can now bind a new thread and have the other thread run some code
        // bound to the hub we just created.
        thread::spawn(move || {
            sentry::Hub::run(hub, || {
                // the log integration picks up the Hub::current which is now bound
                // to the outer hub.
                log::error!("Failing!");
            });
        })
        .join()
        .unwrap();
    })
    .join()
    .unwrap();
}
