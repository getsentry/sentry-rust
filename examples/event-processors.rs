extern crate sentry;

fn main() {
    let client =
        sentry::Client::from_config("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");
    let _sentry = sentry::init(client);

    sentry::Hub::current().add_event_processor(|| {
        let req = sentry::protocol::Request {
            url: Some("https://example.com/".parse().unwrap()),
            method: Some("GET".into()),
            ..Default::default()
        };
        Box::new(move |event| event.request = Some(req.clone()))
    });

    sentry::configure_scope(|scope| {
        scope.set_fingerprint(Some(["a-message"].as_ref()));
        scope.set_tag("foo", "bar");
    });

    let id = sentry::capture_message("An HTTP request failed.", sentry::Level::Error);
    println!("sent event {}", id);
}
