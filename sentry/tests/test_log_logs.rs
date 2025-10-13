#![cfg(feature = "test")]

// Test `log` integration <> Sentry structured logging.
// This must be a in a separate file because `log::set_boxed_logger` can only be called once.
#[cfg(feature = "logs")]
#[test]
fn test_log_logs() {
    let logger = sentry_log::SentryLogger::new().filter(|_| sentry_log::LogFilter::Log);

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        .unwrap();

    let options = sentry::ClientOptions {
        enable_logs: true,
        ..Default::default()
    };

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            log::info!(user_id = 42, request_id = "abc123"; "This is a log");
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

                let info_log = logs
                    .iter()
                    .find(|log| log.level == sentry::protocol::LogLevel::Info)
                    .expect("expected info log");
                assert_eq!(info_log.body, "This is a log");
                assert_eq!(
                    info_log.attributes.get("user_id").unwrap().clone(),
                    42.into()
                );
                assert_eq!(
                    info_log.attributes.get("request_id").unwrap().clone(),
                    "abc123".into()
                );
                assert_eq!(
                    info_log.attributes.get("logger.target").unwrap().clone(),
                    "test_log_logs".into()
                );
                assert_eq!(
                    info_log.attributes.get("sentry.origin").unwrap().clone(),
                    "auto.log.log".into()
                );
            }
            _ => panic!("expected logs"),
        },
        _ => panic!("expected item container"),
    }
}
