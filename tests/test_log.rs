#![cfg(feature = "with_test_support")]
#![cfg(feature = "with_log")]

#[test]
fn test_log() {
    sentry::integrations::log::init(None, Default::default());

    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        log::info!("Hello World!");
        log::error!("Shit's on fire yo");
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(event.breadcrumbs[0].message, Some("Hello World!".into()));
}
