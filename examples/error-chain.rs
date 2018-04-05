#[macro_use]
extern crate error_chain;
extern crate sentry;

use std::{thread::sleep, time::Duration};
use sentry::{integrations::error_chain::capture_error_chain};

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
    sentry::init("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");
    let err = make_err().chain_err(|| "foobar").unwrap_err();
    capture_error_chain(&err);
    sleep(Duration::from_secs(2));
}
