#![cfg(feature = "test")]

use std::sync::Arc;

use sentry::{
    protocol::{Breadcrumb, Context, EnvelopeItem, Level, SpanStatus},
    test::TestTransport,
    ClientOptions, Hub,
};
use sentry_tower::{SentryHttpLayer, SentryLayer};
use tower::{ServiceBuilder, ServiceExt};

#[test]
fn test_tower_http_records_response_status_code() {
    let options = ClientOptions::new().traces_sample_rate(1.0);

    let envelopes = sentry::test::with_captured_envelopes_options(
        || {
            let service = ServiceBuilder::new()
                .layer(SentryHttpLayer::new().enable_transaction())
                .service_fn(|_req: http::Request<()>| async move {
                    Ok::<_, std::convert::Infallible>(
                        http::Response::builder()
                            .status(http::StatusCode::NOT_FOUND)
                            .body(())
                            .unwrap(),
                    )
                });

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let request = http::Request::builder()
                .method(http::Method::GET)
                .uri("http://example.com/missing")
                .body(())
                .unwrap();
            let response = rt.block_on(service.oneshot(request)).unwrap();
            assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
        },
        options,
    );

    assert_eq!(envelopes.len(), 1);
    let transaction = match envelopes[0].items().next().unwrap() {
        EnvelopeItem::Transaction(transaction) => transaction,
        _ => panic!("expected a transaction item"),
    };

    assert_eq!(transaction.name.as_deref(), Some("GET /missing"));
    let Context::Trace(trace) = transaction.contexts.get("trace").unwrap() else {
        panic!("expected a trace context");
    };
    assert_eq!(trace.op.as_deref(), Some("http.server"));
    assert_eq!(trace.origin.as_deref(), Some("auto.http.tower"));
    assert_eq!(trace.status, Some(SpanStatus::NotFound));
    assert_eq!(
        trace.data.get("http.response.status_code"),
        Some(&404.into())
    );
}

#[test]
fn test_tower_hub() {
    // Create a fake transport for new hubs
    let transport = TestTransport::new();
    let opts = ClientOptions::new()
        .dsn("https://public@sentry.invalid/1")
        .transport(transport.clone());

    let events = sentry::test::with_captured_events(|| {
        // This breadcrumb should be in all subsequent requests
        sentry::add_breadcrumb(Breadcrumb {
            message: Some("Starting service...".to_owned()),
            level: Level::Info,
            ..Default::default()
        });
        sentry::capture_message("Started service", Level::Info);

        let hub = Arc::new(Hub::new_from_top(Hub::current()));
        hub.bind_client(Some(Arc::new(opts.into())));

        let service = ServiceBuilder::new()
            .layer(SentryLayer::new(hub))
            .service_fn(|req: String| async move {
                // This breadcrumb should not be seen in any other hub
                sentry::add_breadcrumb(Breadcrumb {
                    message: Some(format!("Got request with arg: {req}")),
                    level: Level::Info,
                    ..Default::default()
                });
                sentry::capture_message("Request failed", Level::Error);
                Err::<(), _>(format!("Can't greet {req}, sorry."))
            });

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let res = rt.block_on(service.oneshot("World".to_owned()));

        assert_eq!(res, Err("Can't greet World, sorry.".to_owned()));
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();
    assert_eq!(event.message, Some("Started service".into()));
    assert_eq!(event.breadcrumbs.len(), 1);
    assert_eq!(
        event.breadcrumbs[0].message,
        Some("Starting service...".into())
    );

    let events = transport.fetch_and_clear_events();
    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();
    assert_eq!(event.message, Some("Request failed".into()));
    assert_eq!(event.breadcrumbs.len(), 2);
    assert_eq!(
        event.breadcrumbs[0].message,
        Some("Starting service...".into())
    );
    assert_eq!(
        event.breadcrumbs[1].message,
        Some("Got request with arg: World".into())
    );
}
