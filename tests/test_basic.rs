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
    assert_eq!(
        event.tags.into_iter().collect::<Vec<(String, String)>>(),
        vec![("worker".to_string(), "worker1".to_string())]
    );
}

#[test]
fn test_breadcrumbs() {
    let events = sentry::test::with_captured_events(|| {
        sentry::add_breadcrumb(|| sentry::Breadcrumb {
            ty: "log".into(),
            message: Some("First breadcrumb".into()),
            ..Default::default()
        });
        sentry::add_breadcrumb(sentry::Breadcrumb {
            ty: "log".into(),
            message: Some("Second breadcrumb".into()),
            ..Default::default()
        });
        sentry::add_breadcrumb(|| {
            vec![
                sentry::Breadcrumb {
                    ty: "log".into(),
                    message: Some("Third breadcrumb".into()),
                    ..Default::default()
                },
                sentry::Breadcrumb {
                    ty: "log".into(),
                    message: Some("Fourth breadcrumb".into()),
                    ..Default::default()
                },
            ]
        });
        sentry::add_breadcrumb(|| None);
        sentry::capture_message("Hello World!", sentry::Level::Warning);
    });
    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    let messages: Vec<_> = event.breadcrumbs.iter().map(|x| {
        (x.message.as_ref().map(|x| x.as_str()).unwrap(), x.ty.as_str())
    }).collect();
    assert_eq!(messages, vec![
        ("First breadcrumb", "log"),
        ("Second breadcrumb", "log"),
        ("Third breadcrumb", "log"),
        ("Fourth breadcrumb", "log"),
    ]);
}
