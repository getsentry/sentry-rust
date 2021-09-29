#![cfg(feature = "test")]

use log_ as log;
use tracing_ as tracing;
use tracing_subscriber::prelude::*;

#[test]
fn test_tracing() {
    // Don't configure the fmt layer to avoid logging to test output
    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_tracing::layer())
        .set_default();

    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        tracing::info!("Hello Tracing World!");
        tracing::error!("Shit's on fire yo");

        log::info!("Hello Logging World!");
        log::error!("Shit's on fire yo");
    });

    assert_eq!(events.len(), 2);
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
}

#[tracing::instrument(fields(span_field))]
fn function() {
    tracing::Span::current().record("span_field", &"some data");
}

#[test]
fn test_span_record() {
    let _dispatcher = tracing_subscriber::registry()
        .with(sentry_tracing::layer())
        .set_default();

    let options = sentry::ClientOptions{
        traces_sample_rate: 1.0,
        ..Default::default()
    };

    let envelopes = sentry::test::with_captured_envelopes_options(|| {
        let _span = tracing::span!(tracing::Level::INFO, "span").entered();
        function();
    }, options);

    assert_eq!(envelopes.len(), 1);

    let envelope_item = envelopes[0].items().next().unwrap();
    let ref transaction = match envelope_item {
        sentry::protocol::EnvelopeItem::Transaction(t) => t,
        _ => { assert!(false); return; }
    };

    assert_eq!(transaction.spans[0].data["span_field"].as_str().unwrap(), "some data");
}