extern crate log;
extern crate pretty_env_logger;
extern crate sentry;

use log::{debug, error, info, warn};
use sentry::sentry_crate_release;

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry_crate_release!(),
            ..Default::default()
        },
    ));

    let mut log_builder = pretty_env_logger::formatted_builder();
    log_builder.parse("info");
    sentry::integrations::env_logger::init(Some(log_builder.build()), Default::default());
    sentry::integrations::panic::register_panic_handler();

    debug!("System is booting");
    info!("System is booting");
    warn!("System is warning");
    error!("Holy shit everything is on fire!");
}
