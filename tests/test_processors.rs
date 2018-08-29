extern crate sentry;

#[test]
fn test_event_processors() {
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
            scope.add_event_processor(Box::new(move |mut event| {
                event.user = Some(sentry::User {
                    email: Some("foo@example.com".into()),
                    ..Default::default()
                });
                Some(event)
            }));
        });
        sentry::capture_message("Hello World!", sentry::Level::Warning);
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(
        event.user,
        Some(sentry::User {
            email: Some("foo@example.com".into()),
            ..Default::default()
        })
    );
}
