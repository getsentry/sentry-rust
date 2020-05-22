#![cfg(feature = "test")]

use log_ as log;

#[test]
fn test_log() {
    let log_integration = sentry_log::LogIntegration::default();

    let events = sentry::test::with_captured_events_options(
        || {
            sentry::configure_scope(|scope| {
                scope.set_tag("worker", "worker1");
            });

            log::info!("Hello World!");
            log::error!("Shit's on fire yo");
        },
        sentry::ClientOptions::default().add_integration(log_integration),
    );

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(event.breadcrumbs[0].message, Some("Hello World!".into()));
}
