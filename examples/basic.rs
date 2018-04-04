extern crate sentry;
#[macro_use] extern crate futures;

use sentry::{Client, protocol::Event};

task_local! {
    static FOO: u32 = 0
}

fn main() {
    let event = Event {
        message: Some("hello, world!".into()),
        ..Default::default()
    };

    let client = Client::new(
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156"
            .parse()
            .unwrap(),
    );

    /*
    let id = client.capture_event(event);
    println!("sent event: {}", id);
    */
}
