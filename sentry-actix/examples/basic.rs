use std::env;
use std::io;

use actix_web::{get, App, Error, HttpRequest, HttpServer};

#[get("/")]
async fn failing(_req: HttpRequest) -> Result<String, Error> {
    Err(io::Error::new(io::ErrorKind::Other, "An error happens here").into())
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    let _guard = sentry::init(());
    env::set_var("RUST_BACKTRACE", "1");

    HttpServer::new(|| {
        App::new()
            .wrap(sentry_actix::Sentry::new())
            .service(failing)
    })
    .bind("127.0.0.1:3001")?
    .run()
    .await?;

    Ok(())
}
