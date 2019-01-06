extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate sentry;

use sentry::integrations::failure::capture_error;

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
    }
    .into())
}

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry_crate_release!(),
            ..Default::default()
        },
    ));

    if let Err(err) = execute() {
        println!("error: {}", err);
        capture_error(&err);
    }
}
