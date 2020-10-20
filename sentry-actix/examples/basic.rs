use std::env;
use std::io;

use actix_web::{get, App, Error, HttpRequest, HttpServer};
use sentry::Level;

#[get("/")]
async fn failing(_req: HttpRequest) -> Result<String, Error> {
    Err(io::Error::new(io::ErrorKind::Other, "An error happens here").into())
}

#[get("/hello")]
async fn hello_world(_req: HttpRequest) -> Result<String, Error> {
    sentry::capture_message("Something is not well", Level::Warning);
    Ok("Hello World".into())
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let _guard = sentry::init(());
    env::set_var("RUST_BACKTRACE", "1");

    HttpServer::new(|| {
        App::new()
            .wrap(sentry_actix::Sentry::new())
            .service(failing)
            .service(hello_world)
    })
    .bind("127.0.0.1:3001")?
    .run()
    .await?;

    Ok(())
}
