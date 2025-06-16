#![cfg(feature = "test")]

use sentry::protocol::{Context, Request, Value};
use tracing_subscriber::prelude::*;

#[test]
fn test_tracing() {
    // Don't configure the fmt layer to avoid logging to test output
    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_tracing::layer())
        .set_default();

    #[tracing::instrument(err)]
    fn fn_errors() -> Result<(), Box<dyn std::error::Error>> {
        Err("I'm broken!".into())
    }

    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        tracing::info!("Hello Tracing World!");
        tracing::error!(tagname = "tagvalue", "Shit's on fire yo");

        log::info!("Hello Logging World!");
        log::error!("Shit's on fire yo");

        let err = "NaN".parse::<usize>().unwrap_err();
        let err: &dyn std::error::Error = &err;
        tracing::warn!(something = err, "Breadcrumb with error");
        tracing::error!(err, tagname = "tagvalue");
        let _ = fn_errors();
    });

    assert_eq!(events.len(), 4);
    let mut events = events.into_iter();

    let event = events.next().unwrap();
    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.message, Some("Shit's on fire yo".to_owned()));
    assert_eq!(event.breadcrumbs.len(), 1);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(
        event.breadcrumbs[0].message,
        Some("Hello Tracing World!".into())
    );
    match event.contexts.get("Rust Tracing Fields").unwrap() {
        Context::Other(tags) => {
            let value = Value::String("tagvalue".to_string());
            assert_eq!(*tags.get("tagname").unwrap(), value);
        }
        _ => panic!("Wrong context type"),
    }
    match event.contexts.get("Rust Tracing Location").unwrap() {
        Context::Other(tags) => {
            assert!(matches!(tags.get("module_path").unwrap(), Value::String(_)));
            assert!(matches!(tags.get("file").unwrap(), Value::String(_)));
            assert!(matches!(tags.get("line").unwrap(), Value::Number(_)));
        }
        _ => panic!("Wrong context type"),
    }

    let event = events.next().unwrap();
    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.message, Some("Shit's on fire yo".to_owned()));
    assert_eq!(event.breadcrumbs.len(), 2);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(
        event.breadcrumbs[0].message,
        Some("Hello Tracing World!".into())
    );
    assert_eq!(event.breadcrumbs[1].level, sentry::Level::Info);
    assert_eq!(
        event.breadcrumbs[1].message,
        Some("Hello Logging World!".into())
    );

    let event = events.next().unwrap();
    assert_eq!(event.breadcrumbs.len(), 3);
    assert!(!event.exception.is_empty());
    assert_eq!(event.exception[0].ty, "ParseIntError");
    assert_eq!(
        event.exception[0].value,
        Some("invalid digit found in string".into())
    );
    match event.contexts.get("Rust Tracing Fields").unwrap() {
        Context::Other(tags) => {
            let value = Value::String("tagvalue".to_string());
            assert_eq!(*tags.get("tagname").unwrap(), value);
        }
        _ => panic!("Wrong context type"),
    }
    match event.contexts.get("Rust Tracing Location").unwrap() {
        Context::Other(tags) => {
            assert!(matches!(tags.get("module_path").unwrap(), Value::String(_)));
            assert!(matches!(tags.get("file").unwrap(), Value::String(_)));
            assert!(matches!(tags.get("line").unwrap(), Value::Number(_)));
        }
        _ => panic!("Wrong context type"),
    }

    assert_eq!(event.breadcrumbs[2].level, sentry::Level::Warning);
    assert_eq!(
        event.breadcrumbs[2].message,
        Some("Breadcrumb with error".into())
    );
    assert!(event.breadcrumbs[2].data.contains_key("something"));
    assert_eq!(
        event.breadcrumbs[2].data.get("something").unwrap(),
        &Value::from(vec!("ParseIntError: invalid digit found in string"))
    );

    let event = events.next().unwrap();
    assert_eq!(event.message, Some("I'm broken!".to_string()));
}

#[tracing::instrument(fields(span_field))]
fn function() {
    tracing::Span::current().record("span_field", "some data");
}

#[test]
fn test_span_record() {
    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_tracing::layer())
        .set_default();

    let options = sentry::ClientOptions {
        traces_sample_rate: 1.0,
        ..Default::default()
    };

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let _span = tracing::span!(tracing::Level::INFO, "span").entered();
            function();
        },
        options,
    );

    assert_eq!(envelopes.len(), 1);

    let envelope_item = envelopes[0].items().next().unwrap();
    let transaction = match envelope_item {
        sentry::protocol::EnvelopeItem::Transaction(t) => t,
        _ => panic!("expected only a transaction item"),
    };

    assert_eq!(
        transaction.spans[0].data["span_field"].as_str().unwrap(),
        "some data"
    );
}

#[test]
fn test_set_transaction() {
    let options = sentry::ClientOptions {
        traces_sample_rate: 1.0,
        ..Default::default()
    };

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let ctx = sentry::TransactionContext::new("old name", "ye, whatever");
            let trx = sentry::start_transaction(ctx);
            let request = Request {
                url: Some("https://honk.beep".parse().unwrap()),
                method: Some("GET".to_string()),
                ..Request::default()
            };
            trx.set_request(request);

            sentry::configure_scope(|scope| scope.set_span(Some(trx.clone().into())));

            sentry::configure_scope(|scope| scope.set_transaction(Some("new name")));

            trx.finish();
        },
        options,
    );

    assert_eq!(envelopes.len(), 1);

    let envelope_item = envelopes[0].items().next().unwrap();
    let transaction = match envelope_item {
        sentry::protocol::EnvelopeItem::Transaction(t) => t,
        _ => panic!("expected only a transaction item"),
    };

    assert_eq!(transaction.name.as_deref().unwrap(), "new name");
    assert!(transaction.request.is_some());
}

#[cfg(feature = "logs")]
#[test]
fn test_tracing_logs() {
    let sentry_layer = sentry_tracing::layer().event_filter(|_| sentry_tracing::EventFilter::Log);

    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_layer)
        .set_default();

    let options = sentry::ClientOptions {
        enable_logs: true,
        ..Default::default()
    };

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            #[derive(Debug)]
            struct ConnectionError {
                message: String,
                source: Option<std::io::Error>,
            }

            impl std::fmt::Display for ConnectionError {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.message)
                }
            }

            impl std::error::Error for ConnectionError {
                fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                    self.source
                        .as_ref()
                        .map(|e| e as &(dyn std::error::Error + 'static))
                }
            }

            let io_error =
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
            let connection_error = ConnectionError {
                message: "Failed to connect to database server".to_string(),
                source: Some(io_error),
            };

            tracing::error!(
                my.key = "hello",
                an.error = &connection_error as &dyn std::error::Error,
                "This is an error log: {}",
                "hello"
            );
        },
        options,
    );

    assert_eq!(envelopes.len(), 1);
    let envelope = envelopes.first().expect("expected envelope");
    let item = envelope.items().next().expect("expected envelope item");

    match item {
        sentry::protocol::EnvelopeItem::ItemContainer(container) => match container {
            sentry::protocol::ItemContainer::Logs(logs) => {
                assert_eq!(logs.len(), 1);

                let log = &logs[0];
                assert_eq!(log.level, sentry::protocol::LogLevel::Error);
                assert_eq!(log.body, "This is an error log: hello");
                assert!(log.trace_id.is_some());
                assert_eq!(
                    log.attributes.get("my.key").unwrap().clone(),
                    sentry::protocol::LogAttribute::from("hello")
                );
                assert_eq!(
                    log.attributes.get("an.error").unwrap().clone(),
                    sentry::protocol::LogAttribute::from(vec![
                        "ConnectionError: Failed to connect to database server",
                        "Custom: Connection refused"
                    ])
                );
            }
            _ => panic!("expected logs container"),
        },
        _ => panic!("expected item container"),
    }
}
