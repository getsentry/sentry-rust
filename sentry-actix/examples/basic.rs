use std::env;
use std::io;

use actix_web::{server, App, Error, HttpRequest};
use sentry_actix::SentryMiddleware;

fn failing(_req: &HttpRequest) -> Result<String, Error> {
    Err(io::Error::new(io::ErrorKind::Other, "Something went really wrong here").into())
}

fn main() {
    let _guard = sentry::init(());
    env::set_var("RUST_BACKTRACE", "1");

    server::new(|| {
        App::new()
            .middleware(SentryMiddleware::builder().emit_header(true).finish())
            .resource("/", |r| r.f(failing))
    })
    .bind("127.0.0.1:3001")
    .unwrap()
    .run();
}
