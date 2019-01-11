use sentry::sentry_crate_release;
use sentry::integrations::error_chain::capture_error_chain;

error_chain::error_chain! {
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
    let _sentry = sentry::init(sentry::ClientOptions {
        dsn: Some(
            "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156"
                .parse()
                .unwrap(),
        ),
        release: sentry_crate_release!(),
        ..Default::default()
    });

    if let Err(err) = execute() {
        println!("error: {}", err);
        capture_error_chain(&err);
    }
}
