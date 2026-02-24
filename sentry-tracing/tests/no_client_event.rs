use sentry_tracing::layer;
use tracing_subscriber::prelude::*;

#[test]
fn test_events_without_client_do_not_panic() {
    // No Sentry client -- tracing events should be silently ignored.
    let subscriber = tracing_subscriber::registry().with(layer());
    let _guard = tracing::subscriber::set_default(subscriber);

    // These should not panic or do any Sentry work.
    tracing::info!("info message");
    tracing::warn!("warning message");
    tracing::error!("error message");
}
