extern crate failure;
extern crate sentry;

use std::sync::Arc;

use sentry::{bind_client, capture_exception, Client, protocol::Event};
use failure::Error;

fn f(num: u32) -> Result<u32, Error> {
    if (num < 1) {
        return Err(Error::from(failure::err_msg("kaputt")));
    }

    Ok(f(num - 1)? + f(num - 2)?)
}

fn main() {
    let event = Event {
        message: Some("hello, world!".into()),
        ..Default::default()
    };

    let client = Client::new(
        "https://f09df2dafaef4332928a4de20cd45f90@sentry-ja-689a42ff319b.eu.ngrok.io/5"
            .parse()
            .unwrap(),
    );

    bind_client(Arc::new(client));

    capture_exception(Some(&f(32).unwrap_err()));

    // println!(
    //     "{}",
    //     capture_exception(Some(&Error::from(failure::err_msg("Hello!"))))
    // );

    ::std::thread::sleep_ms(2000);

    /*
    let id = client.capture_event(event);
    println!("sent event: {}", id);
    */
}
