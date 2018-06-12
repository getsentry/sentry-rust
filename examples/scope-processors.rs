extern crate sentry;

fn main() {
    let client =
        sentry::Client::from_config("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");
    let _sentry = sentry::init(client);
    sentry::configure_scope(|scope| {
        scope.add_scope_processor(Box::new(|event| {
            event.request = Some(sentry::protocol::Request {
                url: Some("https://example.com/".parse().unwrap()),
                method: Some("GET".into()),
                ..Default::default()
            });
        }));
        scope.set_fingerprint(Some(["a-message"].as_ref()));
        scope.set_tag("foo", "bar");
    });

    let id = sentry::capture_message("An HTTP request failed.", sentry::Level::Error);
    println!("sent event {}", id);
}
