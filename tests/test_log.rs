#[macro_use]
extern crate log;
extern crate sentry;

#[test]
fn test_log() {
    let events = sentry::test::with_captured_events_options(
        || {
            sentry::configure_scope(|scope| {
                scope.set_tag("worker", "worker1");
            });

            info!("Hello World!");
            error!("Shit's on fire yo");
        },
        sentry::ClientOptions::default()
            .add_integration(sentry::integrations::log::LogIntegration::default()),
    );

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(event.breadcrumbs[0].message, Some("Hello World!".into()));
}
