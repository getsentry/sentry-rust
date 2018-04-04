#[macro_use]
extern crate error_chain;
extern crate sentry;

use std::sync::Arc;

use sentry::{bind_client, capture_exception, Client, compat::ErrorChain, protocol::Event};

error_chain! {
    errors {
        Foo {
            description("foo")
        }
    }
}

fn make_err() -> Result<()> {
    Err(ErrorKind::Foo.into())
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
    let err = make_err().chain_err(|| "foobar").unwrap_err();
    capture_exception(Some(&ErrorChain(&err)));
    ::std::thread::sleep_ms(2000);
}
