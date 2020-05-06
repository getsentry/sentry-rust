#[macro_use]
extern crate error_chain;

use sentry_error_chain::{capture_error_chain, ErrorChainIntegration};

error_chain! {
    errors {
        MyCoolError(t: &'static str) {
            description("my cool error happened")
            display("my cool error happened: {}", t)
        }
    }
}

fn execute() -> Result<()> {
    Err(ErrorKind::MyCoolError("Something went really wrong").into())
}

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        }
        .add_integration(ErrorChainIntegration),
    ));

    if let Err(err) = execute() {
        println!("error: {}", err);
        capture_error_chain(&err);
    }
}
