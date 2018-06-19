extern crate actix_web;
extern crate failure;
extern crate sentry;
extern crate uuid;

use std::sync::Arc;
use uuid::Uuid;
use actix_web::middleware::{Middleware, Response, Started};
use actix_web::{Error, HttpMessage, HttpRequest, HttpResponse};
use failure::Fail;
use sentry::integrations::failure::exception_from_single_fail;
use sentry::protocol::{Event, Level};
use sentry::Hub;

/// Reports certain failures to sentry.
pub struct CaptureSentryError;

impl<S: 'static> Middleware<S> for CaptureSentryError {
    fn start(&self, req: &mut HttpRequest<S>) -> Result<Started, Error> {
        let hub = Arc::new(Hub::new_from_top(Hub::current()));
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
                let hub = Hub::from_request(req);
                println!("capturing error");
                hub.capture_actix_error(error);
            }
        }
        Ok(Response::Done(resp))
    }
}

/// Utility function that takes an actix error and reports it to the default hub.
pub fn capture_actix_error(err: &Error) -> Uuid {
    Hub::with_active(|hub| {
        hub.capture_actix_error(err)
    })
}

/// Hub extensions for actix.
pub trait HubExt {
    /// Returns the hub from a given http request.
    fn from_request<S>(req: &HttpRequest<S>) -> &Arc<Hub>;
    /// Captures an actix error on the given hub.
    fn capture_actix_error(&self, err: &Error) -> Uuid;
}

impl HubExt for Hub {
    fn from_request<S>(req: &HttpRequest<S>) -> &Arc<Hub> {
        req.extensions().get().expect("CaptureSentryError middleware was not registered")
    }

    fn capture_actix_error(&self, err: &Error) -> Uuid {
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
        self.capture_event(Event {
            exceptions: exceptions,
            level: Level::Error,
            ..Default::default()
        })
    }
}
