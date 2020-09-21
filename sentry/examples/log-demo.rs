use log_::{debug, error, info, warn};

fn main() {
    let mut log_builder = pretty_env_logger::formatted_builder();
    log_builder.parse_filters("info");
    let log_integration =
        sentry_log::LogIntegration::default().with_env_logger_dest(Some(log_builder.build()));

    let _sentry = sentry::init(
        sentry::ClientOptions::configure(|o| o.set_release(sentry::release_name!()))
            .add_integration(log_integration),
    );

    debug!("System is booting");
    info!("System is booting");
    warn!("System is warning");
    error!("Holy shit everything is on fire!");
}
