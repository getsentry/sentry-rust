extern crate sentry;

use sentry::{Client, protocol::Event};

fn main() {
    let event = Event {
        message: Some("hello, world!".into()),
        ..Default::default()
    };

    let client = Client::new(Some(
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156"
            .parse()
            .unwrap(),
    ));

    let id = client.capture_event(event);
    println!("sent event: {}", id);
}
