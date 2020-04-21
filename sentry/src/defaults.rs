use std::env;
use std::{borrow::Cow, sync::Arc};

use sentry_core::transports::DefaultTransportFactory;

use crate::internals::Dsn;
use crate::utils;
use crate::ClientOptions;

pub fn apply_defaults(mut opts: ClientOptions) -> ClientOptions {
    if opts.transport.is_none() {
        opts.transport = Some(Arc::new(DefaultTransportFactory));
    }
    if opts.dsn.is_none() {
        opts.dsn = env::var("SENTRY_DSN")
            .ok()
            .and_then(|dsn| dsn.parse::<Dsn>().ok());
    }
    if opts.release.is_none() {
        opts.release = env::var("SENTRY_RELEASE").ok().map(Cow::Owned);
    }
    if opts.environment.is_none() {
        opts.environment = env::var("SENTRY_ENVIRONMENT")
            .ok()
            .map(Cow::Owned)
            .or_else(|| {
                Some(Cow::Borrowed(if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }))
            });
    }
    if opts.server_name.is_none() {
        opts.server_name = utils::server_name().map(Cow::Owned);
    }
    if opts.http_proxy.is_none() {
        opts.http_proxy = std::env::var("HTTP_PROXY")
            .ok()
            .map(Cow::Owned)
            .or_else(|| std::env::var("http_proxy").ok().map(Cow::Owned));
    }
    if opts.https_proxy.is_none() {
        opts.https_proxy = std::env::var("HTTPS_PROXY")
            .ok()
            .map(Cow::Owned)
            .or_else(|| std::env::var("https_proxy").ok().map(Cow::Owned))
            .or_else(|| opts.http_proxy.clone());
    }
    opts
}
