use sentry::integrations::std_error::capture_error;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
struct MyError {
    code: u32,
    message: String,
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "An error occurred with error code {}. ({})",
            self.code, self.message,
        )
    }
}

impl Error for MyError {}

fn execute() -> Result<(), MyError> {
    Err(MyError {
        code: 42,
        message: "Something went really wrong".into(),
    })
}

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    if let Err(err) = execute() {
        println!("error: {}", err);
        capture_error(&err);
    }
}
