extern crate actix_web;
extern crate failure;
extern crate sentry;

use std::env;
use std::io;

use actix_web::middleware::{Middleware, Response, Started};
use actix_web::{server, App, Error, HttpMessage, HttpRequest, HttpResponse};
use failure::Fail;
use sentry::integrations::failure::exception_from_single_fail;
use sentry::protocol::{Event, Level};
use sentry::Hub;

/// Reports certain failures to sentry.
pub struct CaptureSentryError;

impl<S: 'static> Middleware<S> for CaptureSentryError {
    fn start(&self, req: &mut HttpRequest<S>) -> Result<Started, Error> {
        let hub = Hub::new_from_top(Hub::current());
        let outer_req = req;
        let req = outer_req.clone();
        hub.add_event_processor(Box::new(move || {
            let resource = req.resource().pattern().to_string();
            let req = sentry::protocol::Request {
                url: format!(
                    "{}://{}{}",
                    req.connection_info().scheme(),
                    req.connection_info().host(),
                    req.uri()
                ).parse()
                    .ok(),
                method: Some(req.method().to_string()),
                headers: req.headers()
                    .iter()
                    .map(|(k, v)| (k.as_str().into(), v.to_str().unwrap_or("").into()))
                    .collect(),
                ..Default::default()
            };
            Box::new(move |event| {
                if event.transaction.is_none() {
                    event.transaction = Some(resource.clone());
                }
                event.request = Some(req.clone());
            })
        }));
        outer_req.extensions_mut().insert(hub);
        Ok(Started::Done)
    }

    fn response(&self, req: &mut HttpRequest<S>, resp: HttpResponse) -> Result<Response, Error> {
        if resp.status().is_server_error() {
            if let Some(error) = resp.error() {
                let hub = req.extensions().get().unwrap();
                report_actix_error_to_sentry(error, hub);
            }
        }
        Ok(Response::Done(resp))
    }
}

pub fn report_actix_error_to_sentry(err: &Error, hub: &Hub) {
    let mut exceptions = vec![];
    let mut ptr: Option<&Fail> = Some(err.as_fail());
    let mut idx = 0;
    while let Some(fail) = ptr {
        exceptions.push(exception_from_single_fail(
            fail,
            if idx == 0 {
                Some(err.backtrace())
            } else {
                fail.backtrace()
            },
        ));
        ptr = fail.cause();
        idx += 1;
    }
    exceptions.reverse();
    hub.capture_event(Event {
        exceptions: exceptions,
        level: Level::Error,
        ..Default::default()
    });
}

fn failing(_req: HttpRequest) -> Result<String, Error> {
    Err(io::Error::new(io::ErrorKind::Other, "Something went really wrong here").into())
}

fn main() {
    let _guard = sentry::init("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");
    env::set_var("RUST_BACKTRACE", "1");
    sentry::integrations::panic::register_panic_handler();

    server::new(|| {
        App::new()
            .middleware(CaptureSentryError)
            .resource("/", |r| r.f(failing))
    }).bind("127.0.0.1:3001")
        .unwrap()
        .run();
}
