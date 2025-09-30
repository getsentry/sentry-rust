#![cfg(feature = "test")]

#[test]
fn test_log() {
    let logger = sentry_log::SentryLogger::new();

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();

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

#[test]
fn test_combined_log_filters() {
    let logger = sentry_log::SentryLogger::new().filter(|md| match md.level() {
        log::Level::Error => sentry_log::LogFilter::Breadcrumb | sentry_log::LogFilter::Event,
        log::Level::Warn => sentry_log::LogFilter::Event,
        _ => sentry_log::LogFilter::Ignore,
    });

    // Only set logger if not already set to avoid conflicts with other tests
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }

    let events = sentry::test::with_captured_events(|| {
        log::error!("Both a breadcrumb and an event");
        log::warn!("An event");
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

#[test]
fn test_combined_record_mapper() {
    let logger = sentry_log::SentryLogger::new().mapper(|record| match record.metadata().level() {
        log::Level::Error => {
            let breadcrumb = sentry_log::breadcrumb_from_record(record);
            let sentry_event = sentry_log::event_from_record(record);

            sentry_log::RecordMapping::Combined(
                vec![
                    sentry_log::RecordMapping::Breadcrumb(breadcrumb),
                    sentry_log::RecordMapping::Event(sentry_event),
                ]
                .into(),
            )
        }
        log::Level::Warn => {
            let sentry_event = sentry_log::event_from_record(record);
            sentry_log::RecordMapping::Event(sentry_event)
        }
        _ => sentry_log::RecordMapping::Ignore,
    });

    // Only set logger if not already set to avoid conflicts with other tests
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }

    let events = sentry::test::with_captured_events(|| {
        log::error!("Both a breadcrumb and an event");
        log::warn!("An event");
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

#[test]
fn test_slog() {
    let drain = sentry_slog::SentryDrain::new(slog::Discard);
    let root = slog::Logger::root(drain, slog::o!());

    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        slog::info!(root, "Hello World!");
        slog::error!(root, "Shit's on fire yo");
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(event.breadcrumbs[0].message, Some("Hello World!".into()));
}
