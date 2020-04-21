fn main() {
    let _sentry = sentry::init("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");

    sentry::configure_scope(|scope| {
        scope.add_event_processor(Box::new(move |mut event| {
            event.request = Some(sentry::protocol::Request {
                url: Some("https://example.com/".parse().unwrap()),
                method: Some("GET".into()),
                ..Default::default()
            });
            Some(event)
        }));
    });

    sentry::configure_scope(|scope| {
        scope.set_fingerprint(Some(["a-message"].as_ref()));
        scope.set_tag("foo", "bar");
    });

    let id = sentry::capture_message("An HTTP request failed.", sentry::Level::Error);
    println!("sent event {}", id);
}
