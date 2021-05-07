use tracing_::{debug, error, info, warn};
use tracing_subscriber::prelude::*;

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(sentry_tracing::layer())
        .try_init()
        .unwrap();

    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });

    debug!("System is booting");
    info!("System is booting");
    warn!("System is warning");
    error!("Holy shit everything is on fire!");
}
