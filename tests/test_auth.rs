extern crate chrono;
extern crate sentry_types;
use chrono::{TimeZone, Utc};
use sentry_types::{protocol, Auth, Dsn};

#[test]
fn test_auth_parsing() {
    let auth: Auth = "Sentry sentry_timestamp=1328055286.5, \
                      sentry_client=raven-python/42, \
                      sentry_version=6, \
                      sentry_key=public, \
                      sentry_secret=secret"
        .parse()
        .unwrap();
    assert_eq!(
        auth.timestamp(),
        Some(Utc.ymd(2012, 2, 1).and_hms_milli(0, 14, 46, 500))
    );
    assert_eq!(auth.client_agent(), Some("raven-python/42"));
    assert_eq!(auth.version(), 6);
    assert_eq!(auth.public_key(), "public");
    assert_eq!(auth.secret_key(), Some("secret"));

    assert_eq!(
        auth.to_string(),
        "Sentry sentry_key=public, \
         sentry_version=6, \
         sentry_timestamp=1328055286.5, \
         sentry_client=raven-python/42, \
         sentry_secret=secret"
    );
}

#[test]
fn auth_to_dsn() {
    let url = "https://username:password@domain:8888/23";
    let dsn = url.parse::<Dsn>().unwrap();
    let auth = dsn.to_auth(Some("sentry-rust/1.0"));
    assert_eq!(auth.client_agent(), Some("sentry-rust/1.0"));
    assert_eq!(auth.version(), protocol::LATEST);
    assert_eq!(auth.public_key(), "username");
    assert_eq!(auth.secret_key(), Some("password"));
}
