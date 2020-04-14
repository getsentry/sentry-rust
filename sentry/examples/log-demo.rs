use log::{debug, error, info, warn};

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let mut log_builder = pretty_env_logger::formatted_builder();
    log_builder.parse_filters("info");
    sentry::integrations::env_logger::init(Some(log_builder.build()), Default::default());
    sentry::integrations::panic::register_panic_handler();

    debug!("System is booting");
    info!("System is booting");
    warn!("System is warning");
    error!("Holy shit everything is on fire!");
}
