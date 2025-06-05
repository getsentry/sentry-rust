#![cfg(feature = "test")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sentry::protocol::{Attachment, Context, EnvelopeItem};
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
fn test_event_trace_context_from_propagation_context() {
    let mut last_event_id = None::<Uuid>;
    let mut span = None;
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            span = scope.get_span();
        });
        sentry::capture_message("Hello World!", sentry::Level::Warning);
        last_event_id = sentry::last_event_id();
    });
    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    let trace_context = event.contexts.get("trace");
    assert!(span.is_none());
    assert!(matches!(trace_context, Some(Context::Trace(_))));
}

#[test]
fn test_breadcrumbs() {
    let events = sentry::test::with_captured_events(|| {
        sentry::add_breadcrumb(|| sentry::Breadcrumb {
            ty: "log".into(),
            message: Some("Old breadcrumb to be removed".into()),
            ..Default::default()
        });
        sentry::configure_scope(|scope| scope.clear_breadcrumbs());
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
        fn send_envelope(&self, envelope: sentry::Envelope) {
            let event = envelope.event().unwrap();
            assert_eq!(event.message.as_ref().unwrap(), "test");
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

#[test]
fn test_reentrant_configure_scope() {
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope1| {
            scope1.set_tag("which_scope", "scope1");

            sentry::configure_scope(|scope2| {
                scope2.set_tag("which_scope", "scope2");
            });
        });

        sentry::capture_message("look ma, no deadlock!", sentry::Level::Info);
    });

    assert_eq!(events.len(), 1);
    // well, the "outer" `configure_scope` wins
    assert_eq!(events[0].tags["which_scope"], "scope1");
}

#[test]
fn test_attached_stacktrace() {
    let logger = sentry_log::SentryLogger::new();

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();

    let options = sentry::apply_defaults(sentry::ClientOptions {
        attach_stacktrace: true,
        ..Default::default()
    });
    let events = sentry::test::with_captured_events_options(
        || {
            let error = "thisisnotanumber".parse::<u32>().unwrap_err();
            sentry::capture_error(&error);

            sentry::capture_message("some kind of message", sentry::Level::Info);

            log::error!("Shit's on fire yo");
        },
        options,
    );

    assert_eq!(events.len(), 3);

    let stacktraces = events
        .into_iter()
        .flat_map(|ev| ev.threads.into_iter().filter_map(|thrd| thrd.stacktrace));
    assert_eq!(stacktraces.count(), 3);
}

#[test]
fn test_attachment_sent_from_scope() {
    let envelopes = sentry::test::with_captured_envelopes(|| {
        sentry::with_scope(
            |scope| {
                scope.add_attachment(Attachment {
                    buffer: vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
                    filename: "test-file.bin".to_string(),
                    ..Default::default()
                })
            },
            || sentry::capture_message("test", sentry::Level::Error),
        );
    });

    assert_eq!(envelopes.len(), 1);

    let items = envelopes[0].items().collect::<Vec<_>>();

    assert_eq!(items.len(), 2);
    assert!(matches!(items[1],
        EnvelopeItem::Attachment(attachment)
        if attachment.filename == *"test-file.bin"
        && attachment.buffer == vec![1, 2, 3, 4, 5, 6, 7, 8, 9]
    ));
}

#[cfg(feature = "panic")]
#[test]
fn test_panic_scope_pop() {
    let options = sentry::ClientOptions::new()
        .add_integration(sentry::integrations::panic::PanicIntegration::new());

    let events = sentry::test::with_captured_events_options(
        || {
            // in case someone wants to log the original panics:
            // let next = std::panic::take_hook();
            // std::panic::set_hook(Box::new(move |info| {
            //     dbg!(&info);
            //     println!("{}", std::backtrace::Backtrace::force_capture());
            //     next(info);
            // }));

            let hub = sentry::Hub::current();
            let scope1 = hub.push_scope();
            let scope2 = hub.push_scope();

            let panic = std::panic::catch_unwind(|| {
                drop(scope1);
            });
            assert!(panic.is_err());

            let panic = std::panic::catch_unwind(|| {
                drop(scope2);
            });
            assert!(panic.is_err());
        },
        options,
    );

    assert_eq!(events.len(), 2);
    assert_eq!(&events[0].exception[0].ty, "panic");
    assert_eq!(
        events[0].exception[0].value,
        Some("Popped scope guard out of order".into())
    );
    assert_eq!(&events[1].exception[0].ty, "panic");
    assert_eq!(
        events[1].exception[0].value,
        Some("Popped scope guard out of order".into())
    );
}

#[cfg(feature = "UNSTABLE_logs")]
#[test]
fn test_basic_capture_log() {
    use std::time::SystemTime;

    use sentry::{protocol::Log, protocol::LogAttribute, protocol::Map, Hub};

    let options = sentry::ClientOptions {
        enable_logs: true,
        ..Default::default()
    };
    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let mut attributes: Map<String, LogAttribute> = Map::new();
            attributes.insert("test".into(), "a string".into());
            let log = Log {
                level: sentry::protocol::LogLevel::Warn,
                body: "this is a test".into(),
                trace_id: None,
                timestamp: SystemTime::now(),
                severity_number: None,
                attributes,
            };

            Hub::current().capture_log(log);
        },
        options,
    );
    assert_eq!(envelopes.len(), 1);
    let envelope = envelopes.first().expect("expected envelope");
    let item = envelope.items().next().expect("expected envelope item");
    match item {
        EnvelopeItem::ItemContainer(container) => match container {
            sentry::protocol::ItemContainer::Logs(logs) => {
                let log = logs.iter().next().expect("expected log");
                assert_eq!(sentry::protocol::LogLevel::Warn, log.level);
                assert_eq!("this is a test", log.body);
                assert!(log.trace_id.is_some());
                assert!(log.severity_number.is_none());
                assert!(log.attributes.contains_key("sentry.sdk.name"));
                assert!(log.attributes.contains_key("sentry.sdk.version"));
                assert!(log.attributes.contains_key("test"));
            }
            _ => panic!("expected logs"),
        },
        _ => panic!("expected item container"),
    }
}

#[cfg(feature = "UNSTABLE_logs")]
#[test]
fn test_basic_capture_log_macro_message() {
    use sentry_core::logger_info;

    let options = sentry::ClientOptions {
        enable_logs: true,
        ..Default::default()
    };
    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            logger_info!("Hello, world!");
        },
        options,
    );
    assert_eq!(envelopes.len(), 1);
    let envelope = envelopes.first().expect("expected envelope");
    let item = envelope.items().next().expect("expected envelope item");
    match item {
        EnvelopeItem::ItemContainer(container) => match container {
            sentry::protocol::ItemContainer::Logs(logs) => {
                let log = logs.iter().next().expect("expected log");
                assert_eq!(sentry_core::protocol::LogLevel::Info, log.level);
                assert_eq!("Hello, world!", log.body);
                assert!(log.trace_id.is_some());
                assert!(log.severity_number.is_none());
                assert!(log.attributes.contains_key("sentry.sdk.name"));
                assert!(log.attributes.contains_key("sentry.sdk.version"));
            }
            _ => panic!("expected logs"),
        },
        _ => panic!("expected item container"),
    }
}

#[cfg(feature = "UNSTABLE_logs")]
#[test]
fn test_basic_capture_log_macro_message_formatted() {
    use sentry::protocol::LogAttribute;
    use sentry_core::logger_warn;

    let options = sentry::ClientOptions {
        enable_logs: true,
        ..Default::default()
    };
    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let failed_requests = ["request1", "request2", "request3"];
            logger_warn!(
                "Critical system errors detected for user {}, total failures: {}",
                "test_user",
                failed_requests.len()
            );
        },
        options,
    );
    assert_eq!(envelopes.len(), 1);
    let envelope = envelopes.first().expect("expected envelope");
    let item = envelope.items().next().expect("expected envelope item");
    match item {
        EnvelopeItem::ItemContainer(container) => match container {
            sentry::protocol::ItemContainer::Logs(logs) => {
                let log = logs.iter().next().expect("expected log");
                assert_eq!(sentry_core::protocol::LogLevel::Warn, log.level);
                assert_eq!(
                    "Critical system errors detected for user test_user, total failures: 3",
                    log.body
                );
                assert_eq!(
                    LogAttribute::from(
                        "Critical system errors detected for user {}, total failures: {}"
                    ),
                    log.attributes
                        .get("sentry.message.template")
                        .unwrap()
                        .clone()
                );
                assert_eq!(
                    LogAttribute::from("test_user"),
                    log.attributes
                        .get("sentry.message.parameter.0")
                        .unwrap()
                        .clone()
                );
                assert_eq!(
                    LogAttribute::from(3),
                    log.attributes
                        .get("sentry.message.parameter.1")
                        .unwrap()
                        .clone()
                );
                assert!(log.trace_id.is_some());
                assert!(log.severity_number.is_none());
                assert!(log.attributes.contains_key("sentry.sdk.name"));
                assert!(log.attributes.contains_key("sentry.sdk.version"));
            }
            _ => panic!("expected logs"),
        },
        _ => panic!("expected item container"),
    }
}

#[cfg(feature = "UNSTABLE_logs")]
#[test]
fn test_basic_capture_log_macro_message_with_attributes() {
    use sentry::protocol::LogAttribute;
    use sentry_core::logger_error;

    let options = sentry::ClientOptions {
        enable_logs: true,
        ..Default::default()
    };
    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            logger_error!(
                user.id = "12345",
                user.active = true,
                request.duration = 150,
                success = false,
                "Failed to process request"
            );
        },
        options,
    );
    assert_eq!(envelopes.len(), 1);
    let envelope = envelopes.first().expect("expected envelope");
    let item = envelope.items().next().expect("expected envelope item");
    match item {
        EnvelopeItem::ItemContainer(container) => match container {
            sentry::protocol::ItemContainer::Logs(logs) => {
                let log = logs.iter().next().expect("expected log");
                assert_eq!(sentry_core::protocol::LogLevel::Error, log.level);
                assert_eq!("Failed to process request", log.body);
                assert_eq!(None, log.attributes.get("sentry.message.template"));
                assert!(log.trace_id.is_some());
                assert!(log.severity_number.is_none());
                assert!(log.attributes.contains_key("sentry.sdk.name"));
                assert!(log.attributes.contains_key("sentry.sdk.version"));
                assert_eq!(
                    LogAttribute::from("12345"),
                    log.attributes.get("user.id").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from(true),
                    log.attributes.get("user.active").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from(150u64),
                    log.attributes.get("request.duration").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from(false),
                    log.attributes.get("success").unwrap().clone()
                );
            }
            _ => panic!("expected logs"),
        },
        _ => panic!("expected item container"),
    }
}

#[cfg(feature = "UNSTABLE_logs")]
#[test]
fn test_basic_capture_log_macro_message_formatted_with_attributes() {
    use sentry::protocol::LogAttribute;
    use sentry_core::logger_debug;

    let options = sentry::ClientOptions {
        enable_logs: true,
        ..Default::default()
    };
    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            logger_debug!(
                hello = "test",
                operation.name = "database_query",
                operation.success = true,
                operation.time_ms = 42,
                world = 10,
                "Database query {} completed in {} ms with {} results",
                "users_by_region",
                42,
                15
            );
        },
        options,
    );
    assert_eq!(envelopes.len(), 1);
    let envelope = envelopes.first().expect("expected envelope");
    let item = envelope.items().next().expect("expected envelope item");
    match item {
        EnvelopeItem::ItemContainer(container) => match container {
            sentry::protocol::ItemContainer::Logs(logs) => {
                let log = logs.iter().next().expect("expected log");
                assert_eq!(sentry_core::protocol::LogLevel::Debug, log.level);
                assert_eq!(
                    "Database query users_by_region completed in 42 ms with 15 results",
                    log.body
                );
                assert!(log.trace_id.is_some());
                assert!(log.severity_number.is_none());
                assert_eq!(
                    LogAttribute::from("Database query {} completed in {} ms with {} results",),
                    log.attributes
                        .get("sentry.message.template")
                        .unwrap()
                        .clone()
                );
                assert!(log.attributes.contains_key("sentry.sdk.name"));
                assert!(log.attributes.contains_key("sentry.sdk.version"));
                assert_eq!(
                    LogAttribute::from("test"),
                    log.attributes.get("hello").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from("database_query"),
                    log.attributes.get("operation.name").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from(true),
                    log.attributes.get("operation.success").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from(42u64),
                    log.attributes.get("operation.time_ms").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from(10),
                    log.attributes.get("world").unwrap().clone()
                );
                assert_eq!(
                    LogAttribute::from("Database query {} completed in {} ms with {} results"),
                    log.attributes
                        .get("sentry.message.template")
                        .unwrap()
                        .clone()
                );
                assert_eq!(
                    LogAttribute::from("users_by_region"),
                    log.attributes
                        .get("sentry.message.parameter.0")
                        .unwrap()
                        .clone()
                );
                assert_eq!(
                    LogAttribute::from(42),
                    log.attributes
                        .get("sentry.message.parameter.1")
                        .unwrap()
                        .clone()
                );
                assert_eq!(
                    LogAttribute::from(15),
                    log.attributes
                        .get("sentry.message.parameter.2")
                        .unwrap()
                        .clone()
                );
            }
            _ => panic!("expected logs"),
        },
        _ => panic!("expected item container"),
    }
}
