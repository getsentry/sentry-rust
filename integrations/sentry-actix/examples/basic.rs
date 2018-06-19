extern crate actix_web;
extern crate sentry;
extern crate sentry_actix;

use std::io;
use std::env;

use sentry_actix::CaptureSentryError;
use actix_web::{server, App, Error, HttpRequest};

fn failing(_req: HttpRequest) -> Result<String, Error> {
    Err(io::Error::new(io::ErrorKind::Other, "Something went really wrong here").into())
}

fn main() {
    let _guard = sentry::init("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");
    env::set_var("RUST_BACKTRACE", "1");
    sentry::integrations::panic::register_panic_handler();

    server::new(|| {
        App::new()
            .middleware(CaptureSentryError)
            .resource("/", |r| r.f(failing))
    }).bind("127.0.0.1:3001")
        .unwrap()
        .run();
}
