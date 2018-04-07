extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate sentry;

use sentry::integrations::failure::capture_fail_error;

#[derive(Fail, Debug)]
#[fail(display = "An error occurred with error code {}. ({})", code, message)]
struct MyError {
    code: i32,
    message: String,
}

fn execute() -> Result<(), failure::Error> {
    Err(MyError {
        code: 42,
        message: "Something went really wrong".into(),
    }.into())
}

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: Some("16ebee932f262d6457d8713defc49714159c0a1a".into()),
            ..Default::default()
        },
    ));

    if let Err(err) = execute() {
        println!("error: {}", err);
        capture_fail_error(&err);
    }
}
