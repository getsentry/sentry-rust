#![cfg(feature = "test")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sentry::types::Uuid;

#[test]
fn test_basic_capture_message() {
    let mut last_event_id = None::<Uuid>;
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });
        sentry::capture_message("Hello World!", sentry::Level::Warning);
        last_event_id = sentry::last_event_id();
    });
    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();
    assert_eq!(event.message.unwrap(), "Hello World!");
    assert_eq!(event.level, sentry::Level::Warning);
    assert_eq!(
        event.tags.into_iter().collect::<Vec<(String, String)>>(),
        vec![("worker".to_string(), "worker1".to_string())]
    );

    assert_eq!(Some(event.event_id), last_event_id);
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

    let messages: Vec<_> = event
        .breadcrumbs
        .iter()
        .map(|x| (x.message.as_deref().unwrap(), x.ty.as_str()))
        .collect();
    assert_eq!(
        messages,
        vec![
            ("First breadcrumb", "log"),
            ("Second breadcrumb", "log"),
            ("Third breadcrumb", "log"),
            ("Fourth breadcrumb", "log"),
        ]
    );
}

#[test]
fn test_factory() {
    struct TestTransport(Arc<AtomicUsize>);

    impl sentry::Transport for TestTransport {
        fn send_event(&self, event: sentry::protocol::Event<'static>) {
            assert_eq!(event.message.unwrap(), "test");
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let events = Arc::new(AtomicUsize::new(0));

    let events_for_options = events.clone();
    let options = sentry::ClientOptions {
        dsn: "http://foo@example.com/42".parse().ok(),
        transport: Some(Arc::new(
            move |opts: &sentry::ClientOptions| -> Arc<dyn sentry::Transport> {
                assert_eq!(opts.dsn.as_ref().unwrap().host(), "example.com");
                Arc::new(TestTransport(events_for_options.clone()))
            },
        )),
        ..Default::default()
    };

    sentry::Hub::run(
        Arc::new(sentry::Hub::new(
            Some(Arc::new(options.into())),
            Arc::new(Default::default()),
        )),
        || {
            sentry::capture_message("test", sentry::Level::Error);
        },
    );

    assert_eq!(events.load(Ordering::SeqCst), 1);
}
