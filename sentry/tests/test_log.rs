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

        log::info!(user_id = 42, request_id = "abc123"; "Hello World!");
        log::error!(error_code = 500, retry_count = 3; "Shit's on fire yo");
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    if let Some(sentry::protocol::Context::Other(attributes)) = event.contexts.get("Rust Log Attributes") {
        assert_eq!(attributes.get("error_code"), 500.into());
        assert_eq!(attributes.get("retry_count"), 3.into());
    } else {
        panic!("Expected 'Rust Log Attributes' context to be present");
    }
    
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(event.breadcrumbs[0].message, Some("Hello World!".into()));
    assert_eq!(event.breadcrumbs[0].data.get("user_id"), 42.into());
    assert_eq!(event.breadcrumbs[0].data.get("request_id"), "abc123".into());
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
