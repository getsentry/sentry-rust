extern crate sentry;

use std::sync::Arc;

fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        dsn: "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156"
            .parse()
            .ok(),
        before_send: Some(Arc::new(Box::new(|mut event| {
            event.request = Some(sentry::protocol::Request {
                url: Some("https://example.com/".parse().unwrap()),
                method: Some("GET".into()),
                ..Default::default()
            });
            Some(event)
        }))),
        debug: true,
        ..Default::default()
    });

    let id = sentry::capture_message("An HTTP request failed.", sentry::Level::Error);
    println!("sent event {}", id);
}
