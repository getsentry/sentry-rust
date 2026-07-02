fn main() {
    let _sentry = sentry::init(
        sentry::ClientOptions::new()
            .before_send(|mut event| {
                event.request = Some(sentry::protocol::Request {
                    url: Some("https://example.com/".parse().unwrap()),
                    method: Some("GET".into()),
                    ..Default::default()
                });
                Some(event)
            })
            .debug(true),
    );

    let id = sentry::capture_message("An HTTP request failed.", sentry::Level::Error);
    println!("sent event {id}");
}
