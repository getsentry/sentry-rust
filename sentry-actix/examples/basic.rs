use actix_web::{server, App, Error, HttpRequest};
use sentry_actix::SentryMiddleware;

fn failing(_req: &HttpRequest) -> Result<String, Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Something went really wrong here",
    )
    .into())
}

const USE_REQUEST_MODE_SESSIONS: bool = true;

fn main() {
    let _guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        auto_session_tracking: !USE_REQUEST_MODE_SESSIONS,
        ..Default::default()
    });
    std::env::set_var("RUST_BACKTRACE", "1");

    server::new(|| {
        let middleware = SentryMiddleware::builder()
            .emit_header(true)
            .track_session(USE_REQUEST_MODE_SESSIONS)
            .finish();

        App::new()
            .middleware(middleware)
            .resource("/", |r| r.f(failing))
    })
    .bind("127.0.0.1:3001")
    .unwrap()
    .run();
}
