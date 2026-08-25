#![cfg(feature = "test")]

// Test `slog` integration <> Sentry structured logging.
#[cfg(feature = "logs")]
#[test]
fn test_slog_logs() {
    let drain =
        sentry_slog::SentryDrain::new(slog::Discard).filter(|_| sentry_slog::LevelFilter::Log);
    let root = slog::Logger::root(drain, slog::o!("global_kv" => 1234));

    let options = sentry::ClientOptions::new().enable_logs(true);

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            slog::info!(root, "This is a log"; "user_id" => 42, "request_id" => "abc123");
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
                    info_log.attributes.get("global_kv").unwrap().clone(),
                    1234.into()
                );
                assert_eq!(
                    info_log.attributes.get("sentry.origin").unwrap().clone(),
                    "auto.logger.slog".into()
                );
                assert!(info_log.attributes.contains_key("code.module.name"));
            }
            _ => panic!("expected logs"),
        },
        _ => panic!("expected item container"),
    }
}
