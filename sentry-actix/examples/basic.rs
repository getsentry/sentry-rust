use std::env;
use std::io;

use actix_web::{get, App, Error, HttpRequest, HttpServer};
use sentry::Level;

#[get("/")]
async fn healthy(_req: HttpRequest) -> Result<String, Error> {
    Ok("All good".into())
}

#[get("/err")]
async fn errors(_req: HttpRequest) -> Result<String, Error> {
    Err(io::Error::new(io::ErrorKind::Other, "An error happens here").into())
}

#[get("/msg")]
async fn captures_message(_req: HttpRequest) -> Result<String, Error> {
    sentry::capture_message("Something is not well", Level::Warning);
    Ok("Hello World".into())
}

// cargo run -p sentry-actix --example basic
fn main() -> io::Result<()> {
    let _guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        auto_session_tracking: true,
        traces_sample_rate: 1.0,
        session_mode: sentry::SessionMode::Request,
        debug: true,
        ..Default::default()
    });
    env::set_var("RUST_BACKTRACE", "1");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let addr = "127.0.0.1:3001";

        println!("Starting server on http://{}", addr);

        HttpServer::new(|| {
            App::new()
                .wrap(sentry_actix::Sentry::with_transaction())
                .service(healthy)
                .service(errors)
                .service(captures_message)
        })
        .bind(addr)?
        .run()
        .await
    })
}
