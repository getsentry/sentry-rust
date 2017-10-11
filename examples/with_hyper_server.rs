
extern crate futures;
extern crate tokio_core;
extern crate hyper;
extern crate sentry;

use futures::*;
use futures::future::*;
use futures::stream::Stream;
use tokio_core::reactor::Core;
use hyper::StatusCode;
use hyper::server::{Http, Service, Request, Response};
use sentry::*;

#[derive(Clone)]
struct EmptyService {
    sentry: Sentry,
}

impl Service for EmptyService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Response, Error = hyper::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        let msg = format!("req: \"{} {} {}\"", req.method(), req.uri(), req.version());
        self.sentry.info("test.logger", &msg, None);

        Box::new(ok(Response::new().with_status(StatusCode::Ok)))
    }
}

fn main() {
    let listen_addr_str = "0.0.0.0:8081";
    let creds_str = "https://mypublickey:myprivatekey@myhost/myprojectid";

    let http = Http::new();

    let mut core = Core::new().expect("failed to initialize event loop");
    let handle = core.handle();

    let creds = creds_str.parse::<SentryCredential>().unwrap();
    let sentry = Sentry::from_settings(handle.clone(), Settings::default(), creds);

    let listen_addr = listen_addr_str.parse().unwrap();
    let listener = tokio_core::net::TcpListener::bind(&listen_addr, &handle)
        .expect("failed to bind address");
    let server = listener.incoming().for_each(move |(sock, addr)| {
        http.bind_connection(&handle, sock, addr, EmptyService { sentry: sentry.clone() });
        Ok(())
    });

    core.run(server).expect("failed to run server");
}
