extern crate sentry;

use std::thread;

#[test]
fn test_event_processors() {
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });
        sentry::Hub::current().add_event_processor(Box::new(|| {
            Box::new(|event| {
                event.user = Some(sentry::User {
                    email: Some("foo@example.com".into()),
                    ..Default::default()
                });
            })
        }));
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

#[test]
fn test_non_send_event_processor_other_thread() {
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });
        sentry::Hub::current().add_event_processor(Box::new(|| {
            Box::new(|event| {
                event.user = Some(sentry::User {
                    email: Some("foo@example.com".into()),
                    ..Default::default()
                });
            })
        }));
        let hub = sentry::Hub::current().clone();

        // the event processor is not send, so it should not fire in the
        // other thread.
        thread::spawn(|| {
            sentry::Hub::run(hub, || {
                sentry::capture_message("Hello World!", sentry::Level::Warning);
            });
        }).join()
            .unwrap();
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert!(event.user.is_none());
}

#[test]
fn test_send_event_processor_other_thread() {
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });
        sentry::Hub::current().add_send_event_processor(Box::new(|| {
            Box::new(|event| {
                event.user = Some(sentry::User {
                    email: Some("foo@example.com".into()),
                    ..Default::default()
                });
            })
        }));
        let hub = sentry::Hub::current().clone();

        // the event processor is send, so it should fire in the
        // other thread.
        thread::spawn(|| {
            sentry::Hub::run(hub, || {
                sentry::capture_message("Hello World!", sentry::Level::Warning);
            });
        }).join()
            .unwrap();
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
