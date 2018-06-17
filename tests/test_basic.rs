extern crate sentry;


#[test]
fn test_basic_capture_message() {
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });
        sentry::capture_message("Hello World!", sentry::Level::Warning);
    });
    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();
    assert_eq!(event.message.unwrap(), "Hello World!");
    assert_eq!(event.level, sentry::Level::Warning);
    assert_eq!(event.tags.into_iter().collect::<Vec<(String, String)>>(), vec![
        ("worker".to_string(), "worker1".to_string()),
    ]);
}
