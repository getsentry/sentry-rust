#![cfg(feature = "test")]

// Test `log` integration with combined filters.
// This must be in a separate file because `log::set_boxed_logger` can only be called once.

#[test]
fn test_log_combined_filters() {
    let logger = sentry_log::SentryLogger::new().filter(|md| match md.level() {
        log::Level::Error => sentry_log::LogFilter::Breadcrumb | sentry_log::LogFilter::Event,
        log::Level::Warn => sentry_log::LogFilter::Event,
        _ => sentry_log::LogFilter::Ignore,
    });

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        .unwrap();

    let events = sentry::test::with_captured_events(|| {
        log::error!("Both a breadcrumb and an event");
        log::warn!("An event");
        log::trace!("Ignored");
    });

    assert_eq!(events.len(), 2);

    assert_eq!(
        events[0].message,
        Some("Both a breadcrumb and an event".to_owned())
    );

    assert_eq!(events[1].message, Some("An event".to_owned()));
    assert_eq!(events[1].breadcrumbs.len(), 1);
    assert_eq!(
        events[1].breadcrumbs[0].message,
        Some("Both a breadcrumb and an event".into())
    );
}
