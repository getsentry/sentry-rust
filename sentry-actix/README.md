<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-actix

This crate adds a middleware for [`actix-web`](https://actix.rs/) that captures errors and
report them to `Sentry`.

To use this middleware just configure Sentry and then add it to your actix web app as a
middleware.  Because actix is generally working with non sendable objects and highly concurrent
this middleware creates a new hub per request.  As a result many of the sentry integrations
such as breadcrumbs do not work unless you bind the actix hub.

## Example

```rust
use std::env;
use std::io;

use actix_web::{get, App, Error, HttpRequest, HttpServer};
use sentry::Level;

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
```

## Reusing the Hub

This integration will automatically update the current Hub instance. For example,
the following will capture a message in the current request's Hub:

```rust
use sentry::Level;
sentry::capture_message("Something is not well", Level::Warning);
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
