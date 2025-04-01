<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-actix

This crate adds a middleware for [`actix-web`](https://actix.rs/) that captures errors and
report them to `Sentry`.

To use this middleware just configure Sentry and then add it to your actix web app as a
middleware.  Because actix is generally working with non sendable objects and highly concurrent
this middleware creates a new Hub per request.

## Example

```rust
use std::io;

use actix_web::{get, App, Error, HttpRequest, HttpServer};

#[get("/")]
async fn failing(_req: HttpRequest) -> Result<String, Error> {
    Err(io::Error::new(io::ErrorKind::Other, "An error happens here").into())
}

fn main() -> io::Result<()> {
    let _guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });
    std::env::set_var("RUST_BACKTRACE", "1");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        HttpServer::new(|| {
            App::new()
                .wrap(sentry_actix::Sentry::new())
                .service(failing)
        })
        .bind("127.0.0.1:3001")?
        .run()
        .await
    })
}
```

## Using Release Health

The actix middleware will automatically start a new session for each request
when `auto_session_tracking` is enabled and the client is configured to
use `SessionMode::Request`.

```rust
let _sentry = sentry::init(sentry::ClientOptions {
    release: sentry::release_name!(),
    session_mode: sentry::SessionMode::Request,
    auto_session_tracking: true,
    ..Default::default()
});
```

## Reusing the Hub

This integration will automatically create a new per-request Hub from the main Hub, and update the
current Hub instance. For example, the following in the handler or in any of the subsequent
middleware will capture a message in the current request's Hub:

```rust
sentry::capture_message("Something is not well", sentry::Level::Warning);
```

It is recommended to register the Sentry middleware as the last, i.e. the first to be executed
when processing a request, so that the rest of the processing will run with the correct Hub.

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
