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

#[actix_web::main]
async fn main() -> io::Result<()> {
    let _guard = sentry::init(sentry::ClientOptions {
        auto_session_tracking: true,
        session_mode: sentry::SessionMode::Request,
        ..Default::default()
    });
    env::set_var("RUST_BACKTRACE", "1");

    let addr = "127.0.0.1:3001";

    println!("Starting server on http://{}", addr);

    HttpServer::new(|| {
        App::new()
            .wrap(sentry_actix::Sentry::new())
            .service(healthy)
            .service(errors)
            .service(captures_message)
    })
    .bind(addr)?
    .run()
    .await?;

    Ok(())
}
