<p align="center">
    <a href="https://sentry.io" target="_blank" align="center">
        <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
    </a>
</p>

# Sentry Rust SDK: sentry-tower

Adds support for automatic hub binding for each request received by the Tower server (or client,
though usefulness is limited in this case).

This allows breadcrumbs collected during the request handling to land in a specific hub, and
avoid having them mixed across requests should a new hub be bound at each request.

## Examples

```rust
use sentry_tower::NewSentryLayer;

// Compose a Tower service where each request gets its own Sentry hub
let service = ServiceBuilder::new()
    .layer(NewSentryLayer::<Request>::new_from_top())
    .timeout(Duration::from_secs(30))
    .service(tower::service_fn(|req: Request| format!("hello {}", req)));
```

More customization can be achieved through the `new` function, such as passing a [`Hub`]
directly.

```rust
use sentry::Hub;
use sentry_tower::SentryLayer;

// Create a hub dedicated to web requests
let hub = Arc::new(Hub::with(|hub| Hub::new_from_top(hub)));

// Compose a Tower service
let service = ServiceBuilder::new()
    .layer(SentryLayer::<_, _, Request>::new(hub))
    .timeout(Duration::from_secs(30))
    .service(tower::service_fn(|req: Request| format!("hello {}", req)));
```

The layer can also accept a closure to return a hub depending on the incoming request.

```rust
use sentry::Hub;
use sentry_tower::SentryLayer;

// Compose a Tower service
let hello = Arc::new(Hub::with(|hub| Hub::new_from_top(hub)));
let other = Arc::new(Hub::with(|hub| Hub::new_from_top(hub)));

let service = ServiceBuilder::new()
    .layer(SentryLayer::new(|req: &Request| match req.as_str() {
        "hello" => hello.clone(),
        _ => other.clone(),
    }))
    .timeout(Duration::from_secs(30))
    .service(tower::service_fn(|req: Request| format!("{} world", req)));
```

When using Tonic, the layer can be used directly by the Tonic stack:

```rust
use sentry_tower::NewSentryLayer;
use hello_world::{*, greeter_server::*};

struct GreeterService;

#[tonic::async_trait]
impl Greeter for GreeterService {
    async fn say_hello(&self, req: Request<HelloRequest>) -> Result<Response<HelloReply>, Status> {
        let HelloRequest { name } = req.into_inner();
        if name == "world" {
            capture_anyhow(&anyhow!("Trying to greet a planet"));
            return Err(Status::invalid_argument("Cannot greet a planet"));
        }
        Ok(Response::new(HelloReply { message: format!("Hello {}", name) }))
    }
}

Server::builder()
    .layer(NewSentryLayer::new_from_top())
    .add_service(GreeterServer::new(GreeterService))
    .serve("127.0.0.1:50051".parse().unwrap())
    .await?;
```

## Resources

License: Apache-2.0

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates
