extern crate failure;
extern crate sentry;

use std::{thread::sleep, time::Duration};
use sentry::{integrations::failure::capture_fail_error};
use failure::Error;

fn f(num: u32) -> Result<u32, Error> {
    if num < 1 {
        return Err(Error::from(failure::err_msg("kaputt")));
    }
    Ok(f(num - 1)? + f(num - 2)?)
}

fn main() {
    sentry::init("https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156");
    capture_fail_error(&f(32).unwrap_err());
    sleep(Duration::from_secs(2));
}
