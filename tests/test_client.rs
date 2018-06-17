extern crate sentry;

#[test]
fn test_into_client() {
    let c: sentry::Client = sentry::Client::from_config("https://public@example.com/42").unwrap();
    {
        let dsn = c.dsn().unwrap();
        assert_eq!(dsn.public_key(), "public");
        assert_eq!(dsn.host(), "example.com");
        assert_eq!(dsn.scheme(), sentry::internals::Scheme::Https);
        assert_eq!(dsn.project_id(), 42.into());
    }

    let c: sentry::Client = sentry::Client::from_config(c).unwrap();
    {
        let dsn = c.dsn().unwrap();
        assert_eq!(dsn.public_key(), "public");
        assert_eq!(dsn.host(), "example.com");
        assert_eq!(dsn.scheme(), sentry::internals::Scheme::Https);
        assert_eq!(dsn.project_id(), 42.into());
    }

    let c: sentry::Client = sentry::Client::from_config((
        "https://public@example.com/42",
        sentry::ClientOptions {
            release: Some("foo@1.0".into()),
            ..Default::default()
        },
    )).unwrap();
    {
        let dsn = c.dsn().unwrap();
        assert_eq!(dsn.public_key(), "public");
        assert_eq!(dsn.host(), "example.com");
        assert_eq!(dsn.scheme(), sentry::internals::Scheme::Https);
        assert_eq!(dsn.project_id(), 42.into());
        assert_eq!(&c.options().release.as_ref().unwrap(), &"foo@1.0");
    }

    assert!(sentry::Client::from_config(()).is_none());
    assert!(sentry::Client::disabled().dsn().is_none());
}
