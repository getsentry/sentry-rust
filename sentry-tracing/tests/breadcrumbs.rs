mod shared;

#[test]
fn breadcrumbs_should_capture_span_fields() {
    let transport = shared::init_sentry();

    foo();

    let data = transport.fetch_and_clear_envelopes();
    assert_eq!(data.len(), 2);

    let event = data.first().expect("should have 1 event");
    let event = match event.items().next().unwrap() {
        sentry::protocol::EnvelopeItem::Event(event) => event,
        unexpected => panic!("Expected event, but got {:#?}", unexpected),
    };

    assert_eq!(event.breadcrumbs.len(), 1);
    assert_eq!(
        event.breadcrumbs[0].data["foo:contextual_value"],
        serde_json::Value::from(42)
    );
    assert_eq!(
        event.breadcrumbs[0].message,
        Some("executing foo".to_owned())
    );
}

#[tracing::instrument(fields(contextual_value = 42))]
fn foo() {
    tracing::info!("executing foo");

    tracing::error!("boom!");
}
