extern crate chrono;
extern crate sentry_types;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate uuid;

use chrono::Utc;
use std::borrow::Cow;

use sentry_types::protocol::v7;

fn reserialize(event: &v7::Event) -> v7::Event<'static> {
    let json = serde_json::to_string(event).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn assert_roundtrip(event: &v7::Event) {
    let event_roundtripped = reserialize(event);
    assert_eq!(&event.clone().into_owned(), &event_roundtripped);
}

mod test_event {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_event_defaults() {
        let event: v7::Event = Default::default();

        assert_eq!(event.id, None);
        assert_eq!(event.timestamp, None);
        assert_eq!(event.fingerprint, vec!["{{ default }}".to_string()]);
        assert_eq!(event.platform, "other");
        assert_eq!(event.level, v7::Level::Error);
        assert_eq!(event.sdk_info, None);
    }

    #[test]
    fn test_event_default_vs_new() {
        let event_new = reserialize(&v7::Event::new());
        let event_default = reserialize(&Default::default());

        assert_eq!(event_default.id, None);
        assert_eq!(event_default.timestamp, None);

        assert!(event_new.id.unwrap() != uuid::Uuid::nil());
        assert!(event_new.timestamp.is_some());
    }

    #[test]
    fn test_event_to_string() {
        let event = v7::Event {
            id: "d43e86c9-6e42-4a93-a4fb-da156dd17341".parse().ok(),
            ..Default::default()
        };
        assert_eq!(
            event.to_string(),
            "Event(id: d43e86c9-6e42-4a93-a4fb-da156dd17341)"
        );
    }

    #[test]
    fn test_event_to_string_timestamp() {
        let event = v7::Event {
            id: "d43e86c9-6e42-4a93-a4fb-da156dd17341".parse().ok(),
            timestamp: Some(Utc.ymd(2017, 12, 24).and_hms(8, 12, 0)),
            ..Default::default()
        };
        assert_eq!(
            event.to_string(),
            "Event(id: d43e86c9-6e42-4a93-a4fb-da156dd17341, ts: 2017-12-24 08:12:00 UTC)"
        );
    }

    #[test]
    fn test_event_release_and_dist() {
        let event = v7::Event {
            dist: Some("42".into()),
            release: Some("my.awesome.app-1.0".into()),
            environment: Some("prod".into()),
            ..Default::default()
        };

        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"release\":\"my.awesome.app-1.0\",\"dist\":\"42\",\"environment\":\"prod\"}"
        );
    }

    #[test]
    fn test_transaction() {
        let event = v7::Event {
            message: Some("Hello World!".to_string()),
            transaction: Some("bar::foo".to_string()),
            level: v7::Level::Info,
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"level\":\"info\",\"transaction\":\"bar::foo\",\"message\":\"Hello World!\"}"
        );
    }

    #[test]
    fn test_logger() {
        let mut event: v7::Event = Default::default();
        event.level = v7::Level::Warning;
        event.message = Some("Hello World!".into());
        event.logger = Some("root".into());
        assert_roundtrip(&event);
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            &json,
            "{\"level\":\"warning\",\"message\":\"Hello World!\",\"logger\":\"root\"}"
        );
    }

    #[test]
    fn test_culprit() {
        let event = v7::Event {
            message: Some("Hello World!".to_string()),
            culprit: Some("foo in bar".to_string()),
            level: v7::Level::Info,
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"level\":\"info\",\"culprit\":\"foo in bar\",\"message\":\"Hello World!\"}"
        );
    }
}

mod test_fingerprint {
    use super::*;

    #[test]
    fn test_fingerprint_simple() {
        let mut event: v7::Event = Default::default();
        assert_eq!(serde_json::to_string(&event).unwrap(), "{}");

        event.fingerprint = {
            let mut fp = event.fingerprint.into_owned();
            fp.push("extra".into());
            Cow::Owned(fp)
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"fingerprint\":[\"{{ default }}\",\"extra\"]}"
        );
    }

    #[test]
    fn test_fingerprint_string() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&["fingerprint".into()]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[\"fingerprint\"]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_bool() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&["True".into(), "False".into()]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[true, false]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_number() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&["-22".into()]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[-22]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_float() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&["3".into()]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[3.0]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_float_trunc() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&["3".into()]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[3.5]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_float_strip() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&[]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[-1e100]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_invalid_fallback() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&["{{ default }}".into()]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[\"a\",null,\"d\"]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_empty() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&[]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[]}").unwrap()
        )
    }

    #[test]
    fn test_fingerprint_float_bounds() {
        assert_eq!(
            v7::Event {
                fingerprint: Cow::Borrowed(&[]),
                ..Default::default()
            },
            serde_json::from_str("{\"fingerprint\":[1.7976931348623157e+308]}").unwrap()
        )
    }
}

mod test_values {
    use super::*;

    #[test]
    fn test_values_array() {
        let values = v7::Values {
            values: vec![1, 2, 3],
            other: v7::Map::new(),
        };

        assert_eq!(values, serde_json::from_str("[1,2,3]").unwrap());
        assert_eq!(
            serde_json::to_string(&values).unwrap(),
            "{\"values\":[1,2,3]}".to_string()
        );
    }

    #[test]
    fn test_values_object() {
        let values = v7::Values {
            values: vec![1, 2, 3],
            other: v7::Map::new(),
        };

        assert_eq!(
            values,
            serde_json::from_str("{\"values\":[1,2,3]}").unwrap()
        );

        assert_eq!(
            serde_json::to_string(&values).unwrap(),
            "{\"values\":[1,2,3]}".to_string()
        );
    }

    #[test]
    fn test_values_additional_data() {
        let values = v7::Values {
            values: vec![1, 2, 3],
            other: {
                let mut m = v7::Map::new();
                m.insert("foo".into(), "bar".into());
                m
            },
        };

        assert_eq!(
            values,
            serde_json::from_str("{\"values\":[1,2,3],\"foo\":\"bar\"}").unwrap()
        );

        assert_eq!(
            serde_json::to_string(&values).unwrap(),
            "{\"values\":[1,2,3],\"foo\":\"bar\"}".to_string()
        );
    }

    #[test]
    fn test_values_option() {
        assert_eq!(
            None,
            serde_json::from_str::<Option<v7::Values<u32>>>("null").unwrap()
        );
    }

    #[test]
    fn test_values_empty() {
        assert!(v7::Values::<u32>::new().is_empty());
        assert!(!v7::Values::from(vec![1, 2, 3]).is_empty())
    }
}

mod test_logentry {
    use super::*;

    #[test]
    fn test_logentry_basics() {
        let event = v7::Event {
            logentry: Some(v7::LogEntry {
                message: "Hello %s!".to_string(),
                params: vec!["World".into()],
                other: Default::default(),
            }),
            culprit: Some("foo in bar".to_string()),
            level: v7::Level::Debug,
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"level\":\"debug\",\"culprit\":\"foo in bar\",\"logentry\":{\"message\":\
             \"Hello %s!\",\"params\":[\"World\"]}}"
        );
    }

    #[test]
    fn test_logentry_no_params() {
        let event = v7::Event {
            logentry: Some(v7::LogEntry {
                message: "Hello World!".to_string(),
                params: vec![],
                other: Default::default(),
            }),
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"logentry\":{\"message\":\"Hello World!\"}}"
        );
    }

    #[test]
    fn test_logentry_interface() {
        assert_eq!(
            v7::Event {
                logentry: Some(v7::LogEntry {
                    message: "Hello World!".to_string(),
                    params: vec![],
                    other: Default::default(),
                }),
                ..Default::default()
            },
            serde_json::from_str("{\"sentry.interfaces.Message\":{\"message\":\"Hello World!\"}}")
                .unwrap()
        );
    }
}

#[test]
fn test_modules() {
    let event = v7::Event {
        modules: {
            let mut m = v7::Map::new();
            m.insert("System".into(), "1.0.0".into());
            m
        },
        ..Default::default()
    };
    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"modules\":{\"System\":\"1.0.0\"}}"
    );
}

mod test_repos {
    use super::*;

    #[test]
    fn test_repos() {
        let event = v7::Event {
            repos: {
                let mut m = v7::Map::new();
                m.insert(
                    "/raven".into(),
                    v7::RepoReference {
                        name: "github/raven".into(),
                        prefix: None,
                        revision: None,
                    },
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"repos\":{\"/raven\":{\"name\":\"github/raven\"}}}"
        );
    }

    #[test]
    fn test_repos_with_revision() {
        let event = v7::Event {
            repos: {
                let mut m = v7::Map::new();
                m.insert(
                    "/raven".into(),
                    v7::RepoReference {
                        name: "github/raven".into(),
                        prefix: Some("/".into()),
                        revision: Some("49f45700b5fe606c1bcd9bf0205ecbb83db17f52".into()),
                    },
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"repos\":{\"/raven\":{\"name\":\"github/raven\",\
             \"prefix\":\"/\",\"revision\":\"49f45700b5fe606c1bcd9bf0205ecbb83db17f52\"}}}"
        );
    }
}

mod test_timestamp {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_timestamp_utc() {
        let event = v7::Event {
            timestamp: Some(Utc.ymd(2017, 12, 24).and_hms(8, 12, 0)),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"timestamp\":1514103120}"
        );

        let event: v7::Event =
            serde_json::from_slice(b"{\"timestamp\":\"2014-05-06T07:08:09Z\"}").unwrap();
        assert_eq!(event.timestamp, Some(Utc.ymd(2014, 5, 6).and_hms(7, 8, 9)));
    }

    #[test]
    fn test_timestamp_float() {
        let event = v7::Event {
            timestamp: Some(Utc.ymd(2017, 12, 24).and_hms_milli(8, 12, 0, 500)),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"timestamp\":1514103120.5}"
        );
    }
}

mod test_user {
    use super::*;

    #[test]
    fn test_user_minimal() {
        let event = v7::Event {
            user: Some(v7::User {
                id: Some("8fd5a33b-5b0e-45b2-aff2-9e4f067756ba".into()),
                email: None,
                ip_address: None,
                username: None,
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"user\":{\"id\":\"8fd5a33b-5b0e-45b2-aff2-9e4f067756ba\"}}"
        );
    }

    #[test]
    fn test_user_full() {
        let event = v7::Event {
            user: Some(v7::User {
                id: Some("8fd5a33b-5b0e-45b2-aff2-9e4f067756ba".into()),
                email: Some("foo@example.invalid".into()),
                ip_address: Some("127.0.0.1".parse().unwrap()),
                username: Some("john-doe".into()),
                other: {
                    let mut hm = v7::Map::new();
                    hm.insert("foo".into(), "bar".into());
                    hm
                },
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"user\":{\"id\":\"8fd5a33b-5b0e-45b2-aff2-9e4f067756ba\",\
             \"email\":\"foo@example.invalid\",\"ip_address\":\"127.0.0.1\",\
             \"username\":\"john-doe\",\"foo\":\"bar\"}}"
        );
    }

    #[test]
    fn test_user_ip_address_auto() {
        let event = v7::Event {
            user: Some(v7::User {
                ip_address: Some(v7::IpAddress::Auto),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"user\":{\"ip_address\":\"{{auto}}\"}}"
        );
    }

    #[test]
    fn test_user_interface() {
        assert_eq!(
            v7::Event {
                user: Some(v7::User {
                    id: Some("8fd5a33b-5b0e-45b2-aff2-9e4f067756ba".into()),
                    email: None,
                    ip_address: None,
                    username: None,
                    ..Default::default()
                }),
                ..Default::default()
            },
            serde_json::from_str(
                "{\"sentry.interfaces.User\":{\"id\":\"8fd5a33b-5b0e-45b2-aff2-9e4f067756ba\"}}"
            ).unwrap()
        );
    }
}

mod test_breadcrumbs {
    use super::*;
    use chrono::TimeZone;

    fn event() -> v7::Event<'static> {
        v7::Event {
            breadcrumbs: vec![
                v7::Breadcrumb {
                    timestamp: Utc.ymd(2017, 12, 24).and_hms_milli(8, 12, 0, 713),
                    category: Some("ui.click".into()),
                    message: Some("span.platform-card > li.platform-tile".into()),
                    ..Default::default()
                },
                v7::Breadcrumb {
                    timestamp: Utc.ymd(2017, 12, 24).and_hms_milli(8, 12, 0, 913),
                    ty: "http".into(),
                    category: Some("xhr".into()),
                    data: {
                        let mut m = v7::Map::new();
                        m.insert("url".into(), "/api/0/organizations/foo".into());
                        m.insert("status_code".into(), 200.into());
                        m.insert("method".into(), "GET".into());
                        m
                    },
                    ..Default::default()
                },
            ].into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_breadcrumbs_list() {
        let event = event();
        assert_eq!(
            event,
            serde_json::from_str(
                "{\"breadcrumbs\":[{\"timestamp\":1514103120.713,\"type\":\"default\",\
                 \"category\":\"ui.click\",\"message\":\"span.platform-card > li.platform-tile\"\
                 },{\"timestamp\":1514103120.913,\"type\":\"http\",\"category\":\"xhr\",\"data\"\
                 :{\"url\":\"/api/0/organizations/foo\",\"status_code\":200,\"method\":\"GET\"}}]}"
            ).unwrap()
        );
    }

    #[test]
    fn test_breadcrumbs_values() {
        let event = event();
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"breadcrumbs\":{\"values\":[{\"timestamp\":1514103120.713,\"type\":\"default\",\
             \"category\":\"ui.click\",\"message\":\"span.platform-card > li.platform-tile\"\
             },{\"timestamp\":1514103120.913,\"type\":\"http\",\"category\":\"xhr\",\"data\"\
             :{\"url\":\"/api/0/organizations/foo\",\"status_code\":200,\"method\":\"GET\"}}]}}"
        );
    }

    #[test]
    fn test_breadcrumbs_interface() {
        let event = event();
        assert_eq!(
            event,
            serde_json::from_str(
                "{\"sentry.interfaces.Breadcrumbs\":[{\"timestamp\":1514103120.713,\"type\":\"default\",\
                \"category\":\"ui.click\",\"message\":\"span.platform-card > li.platform-tile\"\
                },{\"timestamp\":1514103120.913,\"type\":\"http\",\"category\":\"xhr\",\"data\"\
                :{\"url\":\"/api/0/organizations/foo\",\"status_code\":200,\"method\":\"GET\"}}]}"
            ).unwrap()
        );
    }
}

mod test_stacktrace {
    use super::*;

    #[test]
    fn test_stacktrace() {
        let event = v7::Event {
            stacktrace: Some(v7::Stacktrace {
                frames: vec![v7::Frame {
                    function: Some("main".into()),
                    location: v7::FileLocation {
                        filename: Some("hello.py".into()),
                        line: Some(1),
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"stacktrace\":{\"frames\":[{\"function\":\"main\",\
             \"filename\":\"hello.py\",\"lineno\":1}]}}"
        );
    }

    #[test]
    fn test_stacktrace_interface() {
        assert_eq!(
            v7::Event {
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        location: v7::FileLocation {
                            filename: Some("hello.py".into()),
                            line: Some(1),
                            ..Default::default()
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            serde_json::from_str(
                "{\"sentry.interfaces.Stacktrace\":{\"frames\":[{\"function\":\"main\",\
                 \"filename\":\"hello.py\",\"lineno\":1}]}}",
            ).unwrap()
        )
    }
}

mod test_template_info {
    use super::*;

    #[test]
    fn test_template_info() {
        let event = v7::Event {
            template_info: Some(v7::TemplateInfo {
                location: v7::FileLocation {
                    filename: Some("hello.html".into()),
                    line: Some(1),
                    ..Default::default()
                },
                source: v7::EmbeddedSources {
                    pre_lines: vec!["foo1".into(), "bar2".into()],
                    current_line: Some("hey hey hey3".into()),
                    post_lines: vec!["foo4".into(), "bar5".into()],
                },
                other: Default::default(),
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"template\":{\"filename\":\"hello.html\",\"lineno\":1,\
             \"pre_context\":[\"foo1\",\"bar2\"],\"context_line\":\
             \"hey hey hey3\",\"post_context\":[\"foo4\",\"bar5\"]}}"
        );
    }

    #[test]
    fn test_template_info_interface() {
        assert_eq!(
            v7::Event {
                template_info: Some(v7::TemplateInfo {
                    location: v7::FileLocation {
                        filename: Some("hello.html".into()),
                        line: Some(1),
                        ..Default::default()
                    },
                    source: v7::EmbeddedSources {
                        pre_lines: vec!["foo1".into(), "bar2".into()],
                        current_line: Some("hey hey hey3".into()),
                        post_lines: vec!["foo4".into(), "bar5".into()],
                    },
                    other: Default::default(),
                }),
                ..Default::default()
            },
            serde_json::from_str(
                "{\"sentry.interfaces.Template\":{\"filename\":\"hello.html\",\"lineno\":1,\
                 \"pre_context\":[\"foo1\",\"bar2\"],\"context_line\":\
                 \"hey hey hey3\",\"post_context\":[\"foo4\",\"bar5\"]}}",
            ).unwrap()
        )
    }
}

mod test_threads {
    use super::*;

    #[test]
    fn test_threads_values() {
        let event = v7::Event {
            threads: vec![v7::Thread {
                id: Some("#1".into()),
                name: Some("Awesome Thread".into()),
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"threads\":{\"values\":[{\"id\":\"#1\",\"name\":\"Awesome Thread\"}]}}"
        );
    }

    #[test]
    fn test_threads_list() {
        let event = v7::Event {
            threads: vec![v7::Thread {
                id: Some("#1".into()),
                name: Some("Awesome Thread".into()),
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_eq!(
            event,
            serde_json::from_str("{\"threads\":[{\"id\":\"#1\",\"name\":\"Awesome Thread\"}]}")
                .unwrap()
        );
    }

    #[test]
    fn test_threads_interface() {
        let event = v7::Event {
            threads: vec![v7::Thread {
                id: Some("#1".into()),
                name: Some("Awesome Thread".into()),
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_eq!(
            event,
            serde_json::from_str(
                "{\"sentry.interfaces.Threads\":[{\"id\":\"#1\",\"name\":\"Awesome Thread\"}]}"
            ).unwrap()
        );
    }

    #[test]
    fn test_threads_flags() {
        let event = v7::Event {
            threads: vec![v7::Thread {
                id: Some(42.into()),
                name: Some("Awesome Thread".into()),
                crashed: true,
                current: true,
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"threads\":{\"values\":[{\"id\":42,\"name\":\"Awesome Thread\",\
             \"crashed\":true,\"current\":true}]}}"
        );
    }

    #[test]
    fn test_threads_stacktrace() {
        let event = v7::Event {
            threads: vec![v7::Thread {
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        location: v7::FileLocation {
                            filename: Some("hello.py".into()),
                            line: Some(1),
                            ..Default::default()
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                raw_stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        location: v7::FileLocation {
                            filename: Some("hello.py".into()),
                            line: Some(1),
                            ..Default::default()
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"threads\":{\"values\":[{\"stacktrace\":{\"frames\":[{\"function\":\
             \"main\",\"filename\":\"hello.py\",\"lineno\":1}]},\"raw_stacktrace\"\
             :{\"frames\":[{\"function\":\"main\",\"filename\":\"hello.py\",\"lineno\":1}]}}]}}"
        );
    }
}

mod test_request {
    use super::*;

    #[test]
    fn test_request_full() {
        let event = v7::Event {
            request: Some(v7::Request {
                url: "https://www.example.invalid/bar".parse().ok(),
                method: Some("GET".into()),
                data: Some("{}".into()),
                query_string: Some("foo=bar&blub=blah".into()),
                cookies: Some("dummy=42".into()),
                headers: {
                    let mut hm = v7::Map::new();
                    hm.insert("Content-Type".into(), "text/plain".into());
                    hm
                },
                env: {
                    let mut env = v7::Map::new();
                    env.insert("PATH_INFO".into(), "/bar".into());
                    env
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"request\":{\"url\":\"https://www.example.invalid/bar\",\
             \"method\":\"GET\",\"data\":\"{}\",\"query_string\":\
             \"foo=bar&blub=blah\",\"cookies\":\"dummy=42\",\"headers\":\
             {\"Content-Type\":\"text/plain\"},\"env\":\
             {\"PATH_INFO\":\"/bar\"}}}"
        );
    }

    #[test]
    fn test_request_other() {
        let event = v7::Event {
            request: Some(v7::Request {
                url: "https://www.example.invalid/bar".parse().ok(),
                method: Some("GET".into()),
                data: Some("{}".into()),
                query_string: Some("foo=bar&blub=blah".into()),
                cookies: Some("dummy=42".into()),
                other: {
                    let mut m = v7::Map::new();
                    m.insert("other_key".into(), "other_value".into());
                    m
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"request\":{\"url\":\"https://www.example.invalid/bar\",\
             \"method\":\"GET\",\"data\":\"{}\",\"query_string\":\
             \"foo=bar&blub=blah\",\"cookies\":\"dummy=42\",\
             \"other_key\":\"other_value\"}}"
        );
    }

    #[test]
    fn test_request_defaults() {
        let event = v7::Event {
            request: Some(Default::default()),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(serde_json::to_string(&event).unwrap(), "{\"request\":{}}");
    }

    #[test]
    fn test_request_interface() {
        assert_eq!(
            v7::Event {
                request: Some(Default::default()),
                ..Default::default()
            },
            serde_json::from_str("{\"sentry.interfaces.Http\":{}}").unwrap()
        )
    }
}

#[test]
fn test_tags() {
    let event = v7::Event {
        tags: {
            let mut m = v7::Map::new();
            m.insert("device_type".into(), "mobile".into());
            m.insert("interpreter".into(), "7".into());
            m
        },
        ..Default::default()
    };

    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"tags\":{\"device_type\":\"mobile\",\"interpreter\":\"7\"}}"
    );
}

#[test]
fn test_extra() {
    let event = v7::Event {
        extra: {
            let mut m = v7::Map::new();
            m.insert(
                "component_state".into(),
                json!({
                "dirty": true,
                "revision": 17
            }),
            );
            m
        },
        ..Default::default()
    };

    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"extra\":{\"component_state\":{\"dirty\":true,\"revision\":17}}}"
    );
}

mod test_debug_meta {
    use super::*;

    #[test]
    fn test_debug_meta() {
        let event = v7::Event {
            debug_meta: Cow::Owned(v7::DebugMeta {
                sdk_info: Some(v7::SystemSdkInfo {
                    sdk_name: "iOS".into(),
                    version_major: 10,
                    version_minor: 3,
                    version_patchlevel: 0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"debug_meta\":{\"sdk_info\":{\"sdk_name\":\"iOS\",\"version_major\":10,\
             \"version_minor\":3,\"version_patchlevel\":0}}}"
        );
    }

    #[test]
    fn test_debug_meta_interface() {
        assert_eq!(
            v7::Event {
                debug_meta: Cow::Owned(v7::DebugMeta {
                    sdk_info: Some(v7::SystemSdkInfo {
                        sdk_name: "iOS".into(),
                        version_major: 10,
                        version_minor: 3,
                        version_patchlevel: 0,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            serde_json::from_str(
                "{\"sentry.interfaces.DebugMeta\":{\"sdk_info\":{\"sdk_name\":\"iOS\",\
                 \"version_major\":10,\"version_minor\":3,\"version_patchlevel\":0}}}"
            ).unwrap()
        );
    }

    #[test]
    fn test_debug_meta_images() {
        let event = v7::Event {
            debug_meta: Cow::Owned(v7::DebugMeta {
                images: vec![
                    v7::AppleDebugImage {
                        name: "CoreFoundation".into(),
                        arch: Some("arm64".into()),
                        cpu_type: Some(1233),
                        cpu_subtype: Some(3),
                        image_addr: 0.into(),
                        image_size: 4096,
                        image_vmaddr: 32768.into(),
                        uuid: "494f3aea-88fa-4296-9644-fa8ef5d139b6".parse().unwrap(),
                    }.into(),
                    v7::SymbolicDebugImage {
                        name: "CoreFoundation".into(),
                        arch: Some("arm64".into()),
                        image_addr: 0.into(),
                        image_size: 4096,
                        image_vmaddr: 32768.into(),
                        id: "494f3aea-88fa-4296-9644-fa8ef5d139b6-1234".parse().unwrap(),
                    }.into(),
                    v7::ProguardDebugImage {
                        uuid: "8c954262-f905-4992-8a61-f60825f4553b".parse().unwrap(),
                    }.into(),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"debug_meta\":{\"images\":[{\"name\":\"CoreFoundation\",\"arch\":\
             \"arm64\",\"cpu_type\":1233,\"cpu_subtype\":3,\"image_addr\":\"0x0\",\
             \"image_size\":4096,\"image_vmaddr\":\"0x8000\",\"uuid\":\
             \"494f3aea-88fa-4296-9644-fa8ef5d139b6\",\"type\":\"apple\"},\
             {\"name\":\"CoreFoundation\",\"arch\":\"arm64\",\"image_addr\":\
             \"0x0\",\"image_size\":4096,\"image_vmaddr\":\"0x8000\",\"id\":\
             \"494f3aea-88fa-4296-9644-fa8ef5d139b6-1234\",\"type\":\"symbolic\"}\
             ,{\"uuid\":\"8c954262-f905-4992-8a61-f60825f4553b\",\"type\":\"proguard\"}]}}"
        );
    }
}

mod test_exception {
    use super::*;

    #[test]
    fn test_exception_null() {
        let event: v7::Event = serde_json::from_slice(b"{\"exception\":null}").unwrap();
        assert_eq!(event.exceptions, Default::default());
    }

    #[test]
    fn test_exception_single() {
        let json = "{\"exception\":{\"type\":\"ZeroDivisionError\"}}";
        let event: v7::Event = serde_json::from_str(&json).unwrap();
        let mut ref_event: v7::Event = Default::default();
        ref_event.exceptions.values.push(v7::Exception {
            ty: "ZeroDivisionError".into(),
            ..Default::default()
        });
        assert_eq!(event, ref_event);
    }

    #[test]
    fn test_exception_list() {
        let json = "{\"exception\":[{\"type\":\"ZeroDivisionError\"}]}";
        let event: v7::Event = serde_json::from_str(&json).unwrap();
        let mut ref_event: v7::Event = Default::default();
        ref_event.exceptions.values.push(v7::Exception {
            ty: "ZeroDivisionError".into(),
            ..Default::default()
        });
        assert_roundtrip(&event);
        assert_eq!(event, ref_event);
    }

    #[test]
    fn test_exception_values() {
        let mut event: v7::Event = Default::default();
        event.exceptions.values.push(v7::Exception {
            ty: "ZeroDivisionError".into(),
            ..Default::default()
        });
        let json = serde_json::to_string(&event).unwrap();
        assert_roundtrip(&event);
        assert_eq!(
            json,
            "{\"exception\":{\"values\":[{\"type\":\"ZeroDivisionError\"}]}}"
        );

        let event2: v7::Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, event2);
    }

    #[test]
    fn test_exception_interface() {
        let json = "{\"sentry.interfaces.Exception\":{\"type\":\"ZeroDivisionError\"}}";
        let event: v7::Event = serde_json::from_str(&json).unwrap();
        let mut ref_event: v7::Event = Default::default();
        ref_event.exceptions.values.push(v7::Exception {
            ty: "ZeroDivisionError".into(),
            ..Default::default()
        });
        assert_eq!(event, ref_event);
    }

    #[test]
    fn test_exception_precedence() {
        let json = "{\"exception\":{\"type\":\"ZeroDivisionError\"},\
                    \"sentry.interfaces.Exception\":{\"type\":\"WRONG\"}}";
        let event: v7::Event = serde_json::from_str(&json).unwrap();
        let mut ref_event: v7::Event = Default::default();
        ref_event.exceptions.values.push(v7::Exception {
            ty: "ZeroDivisionError".into(),
            ..Default::default()
        });
        assert_eq!(event, ref_event);
    }

    #[test]
    fn test_exception_stacktrace_minimal() {
        let event: v7::Event = v7::Event {
            exceptions: vec![v7::Exception {
                ty: "DivisionByZero".into(),
                value: Some("integer division or modulo by zero".into()),
                module: None,
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        location: v7::FileLocation {
                            filename: Some("hello.py".into()),
                            line: Some(1),
                            ..Default::default()
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                raw_stacktrace: None,
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\
             \"value\":\"integer division or modulo by zero\",\"stacktrace\":\
             {\"frames\":[{\"function\":\"main\",\"filename\":\"hello.py\",\
             \"lineno\":1}]}}]}}"
        );
    }

    #[test]
    fn test_exception_stacktrace_larger() {
        let event: v7::Event = v7::Event {
            exceptions: vec![v7::Exception {
                ty: "DivisionByZero".into(),
                value: Some("integer division or modulo by zero".into()),
                module: None,
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        location: v7::FileLocation {
                            filename: Some("hello.py".into()),
                            line: Some(7),
                            column: Some(42),
                            ..Default::default()
                        },
                        source: v7::EmbeddedSources {
                            pre_lines: vec!["foo".into(), "bar".into()],
                            current_line: Some("hey hey hey".into()),
                            post_lines: vec!["foo".into(), "bar".into()],
                        },
                        in_app: Some(true),
                        vars: {
                            let mut m = v7::Map::new();
                            m.insert("var".into(), "value".into());
                            m
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                raw_stacktrace: None,
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\"value\":\
             \"integer division or modulo by zero\",\"stacktrace\":{\"frames\":\
             [{\"function\":\"main\",\"filename\":\"hello.py\",\"lineno\":7,\
             \"colno\":42,\"pre_context\":[\"foo\",\"bar\"],\"context_line\":\
             \"hey hey hey\",\"post_context\":[\"foo\",\"bar\"],\"in_app\":true,\
             \"vars\":{\"var\":\"value\"}}]}}]}}"
        );
    }

    #[test]
    fn test_exception_stacktrace_full() {
        let event: v7::Event = v7::Event {
            exceptions: vec![v7::Exception {
                ty: "DivisionByZero".into(),
                value: Some("integer division or modulo by zero".into()),
                module: Some("x".into()),
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        symbol: Some("main".into()),
                        location: v7::FileLocation {
                            filename: Some("hello.py".into()),
                            abs_path: Some("/app/hello.py".into()),
                            line: Some(7),
                            column: Some(42),
                        },
                        source: v7::EmbeddedSources {
                            pre_lines: vec!["foo".into(), "bar".into()],
                            current_line: Some("hey hey hey".into()),
                            post_lines: vec!["foo".into(), "bar".into()],
                        },
                        in_app: Some(true),
                        vars: {
                            let mut m = v7::Map::new();
                            m.insert("var".into(), "value".into());
                            m
                        },
                        package: Some("hello.whl".into()),
                        module: Some("hello".into()),
                        instruction_info: v7::InstructionInfo {
                            image_addr: Some(v7::Addr(0)),
                            instruction_addr: Some(v7::Addr(0)),
                            symbol_addr: Some(v7::Addr(0)),
                        },
                        other: {
                            let mut m = v7::Map::new();
                            m.insert("zzz".into(), "foo".into());
                            m
                        },
                    }],
                    frames_omitted: Some((1, 2)),
                    registers: {
                        let mut m = v7::Map::new();
                        m.insert("x8".into(), v7::RegVal(0x0));
                        m.insert("x20".into(), v7::RegVal(0x1));
                        m.insert("x21".into(), v7::RegVal(0x1));
                        m.insert("x28".into(), v7::RegVal(0x17025f650));
                        m.insert("x4".into(), v7::RegVal(0x1702eb100));
                        m.insert("x24".into(), v7::RegVal(0x1b1399c20));
                        m.insert("sp".into(), v7::RegVal(0x16fd75060));
                        m.insert("x1".into(), v7::RegVal(0x1b1399bb1));
                        m.insert("x23".into(), v7::RegVal(0x1afe10040));
                        m.insert("x14".into(), v7::RegVal(0x1));
                        m.insert("x19".into(), v7::RegVal(0x0));
                        m.insert("x18".into(), v7::RegVal(0x0));
                        m.insert("x3".into(), v7::RegVal(0x1));
                        m.insert("pc".into(), v7::RegVal(0x18a310ea4));
                        m.insert("x7".into(), v7::RegVal(0x0));
                        m.insert("x10".into(), v7::RegVal(0x57b));
                        m.insert("x6".into(), v7::RegVal(0x0));
                        m.insert("x13".into(), v7::RegVal(0x1));
                        m.insert("x2".into(), v7::RegVal(0x1));
                        m.insert("x27".into(), v7::RegVal(0x1));
                        m.insert("x26".into(), v7::RegVal(0x191ec48d1));
                        m.insert("x9".into(), v7::RegVal(0x1b1399c20));
                        m.insert("x29".into(), v7::RegVal(0x16fd75060));
                        m.insert("x5".into(), v7::RegVal(0x1702eb100));
                        m.insert("fp".into(), v7::RegVal(0x16fd75060));
                        m.insert("x0".into(), v7::RegVal(0x1));
                        m.insert("lr".into(), v7::RegVal(0x18a31aadc));
                        m.insert("x25".into(), v7::RegVal(0x0));
                        m.insert("x16".into(), v7::RegVal(0x18a31aa34));
                        m.insert("x11".into(), v7::RegVal(0x1b3b37b1d));
                        m.insert("cpsr".into(), v7::RegVal(0x20000000));
                        m.insert("x17".into(), v7::RegVal(0x0));
                        m.insert("x15".into(), v7::RegVal(0x881));
                        m.insert("x22".into(), v7::RegVal(0x1b1399bb0));
                        m.insert("x12".into(), v7::RegVal(0x1b3b37b1d));
                        m
                    },
                    ..Default::default()
                }),
                raw_stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        instruction_info: v7::InstructionInfo {
                            image_addr: Some(v7::Addr(0)),
                            instruction_addr: Some(v7::Addr(0)),
                            symbol_addr: Some(v7::Addr(0)),
                        },
                        ..Default::default()
                    }],
                    frames_omitted: Some((1, 2)),
                    ..Default::default()
                }),
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\"value\":\
             \"integer division or modulo by zero\",\"module\":\"x\",\"stacktrace\":\
             {\"frames\":[{\"function\":\"main\",\"symbol\":\"main\",\"module\":\
             \"hello\",\"package\":\"hello.whl\",\"filename\":\"hello.py\",\"abs_path\"\
             :\"/app/hello.py\",\"lineno\":7,\"colno\":42,\"pre_context\":[\"foo\",\"\
             bar\"],\"context_line\":\"hey hey hey\",\"post_context\":[\"foo\",\"bar\"]\
             ,\"in_app\":true,\"vars\":{\"var\":\"value\"},\"image_addr\":\"0x0\",\
             \"instruction_addr\":\"0x0\",\"symbol_addr\":\"0x0\",\"zzz\":\"foo\"}],\"frames_omitted\":\
             [1,2],\"registers\":{\"x8\":\"0x0\",\"x20\":\"0x1\",\"x21\":\"0x1\",\"x28\
             \":\"0x17025f650\",\"x4\":\"0x1702eb100\",\"x24\":\"0x1b1399c20\",\"sp\":\
             \"0x16fd75060\",\"x1\":\"0x1b1399bb1\",\"x23\":\"0x1afe10040\",\"x14\":\
             \"0x1\",\"x19\":\"0x0\",\"x18\":\"0x0\",\"x3\":\"0x1\",\"pc\":\"0x18a310ea4\
             \",\"x7\":\"0x0\",\"x10\":\"0x57b\",\"x6\":\"0x0\",\"x13\":\"0x1\",\"x2\":\
             \"0x1\",\"x27\":\"0x1\",\"x26\":\"0x191ec48d1\",\"x9\":\"0x1b1399c20\",\
             \"x29\":\"0x16fd75060\",\"x5\":\"0x1702eb100\",\"fp\":\"0x16fd75060\",\
             \"x0\":\"0x1\",\"lr\":\"0x18a31aadc\",\"x25\":\"0x0\",\"x16\":\
             \"0x18a31aa34\",\"x11\":\"0x1b3b37b1d\",\"cpsr\":\"0x20000000\",\"x17\":\
             \"0x0\",\"x15\":\"0x881\",\"x22\":\"0x1b1399bb0\",\"x12\":\"0x1b3b37b1d\"}\
             },\"raw_stacktrace\":{\"frames\":[{\"function\":\"main\",\"image_addr\":\
             \"0x0\",\"instruction_addr\":\"0x0\",\"symbol_addr\":\"0x0\"}],\
             \"frames_omitted\":[1,2]}}]}}"
        );
    }

    #[test]
    fn test_exception_mechanism() {
        let event: v7::Event = v7::Event {
            exceptions: vec![v7::Exception {
                ty: "EXC_BAD_ACCESS".into(),
                value: Some("Attempted to dereference garbage pointer 0x1".into()),
                mechanism: Some(v7::Mechanism {
                    ty: "mach".into(),
                    description: None,
                    help_link: Some(
                        "https://developer.apple.com/library/content/qa/qa1367/_index.html"
                            .parse()
                            .unwrap(),
                    ),
                    handled: Some(false),
                    data: {
                        let mut map = v7::Map::new();
                        map.insert("relevant_address".into(), "0x1".into());
                        map
                    },
                    meta: v7::MechanismMeta {
                        errno: Some(v7::CError {
                            number: 2,
                            name: None,
                        }),
                        signal: Some(v7::PosixSignal {
                            number: 11,
                            code: None,
                            name: None,
                            code_name: None,
                        }),
                        mach_exception: Some(v7::MachException {
                            ty: 1,
                            code: 1,
                            subcode: 8,
                            name: None,
                        }),
                        other: Default::default(),
                    },
                    other: Default::default(),
                }),
                ..Default::default()
            }].into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"exception\":{\"values\":[{\"type\":\"EXC_BAD_ACCESS\",\"value\":\"Attempted to \
            dereference garbage pointer 0x1\",\"mechanism\":{\"type\":\"mach\",\"help_link\":\"\
            https://developer.apple.com/library/content/qa/qa1367/_index.html\",\"handled\":false,\"\
            data\":{\"relevant_address\":\"0x1\"},\"meta\":{\"errno\":{\"number\":2},\"signal\":{\"\
            number\":11},\"mach_exception\":{\"exception\":1,\"code\":1,\"subcode\":8}}}}]}}"
        );
    }
}

#[test]
fn test_sdk_info() {
    let event = v7::Event {
        sdk_info: Some(Cow::Owned(v7::ClientSdkInfo {
            name: "sentry-rust".into(),
            version: "1.0".into(),
            integrations: vec!["rocket".into()],
        })),
        ..Default::default()
    };

    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"sdk\":{\"name\":\"sentry-rust\",\"version\":\"1.0\",\
         \"integrations\":[\"rocket\"]}}"
    );
}

#[test]
fn test_other_data() {
    let event = v7::Event {
        id: Some("864ee979-77bf-43ac-96d7-4f7486d138ab".parse().unwrap()),
        other: {
            let mut m = v7::Map::new();
            m.insert("extra_shit".into(), 42.into());
            m.insert("extra_garbage".into(), "aha".into());
            m
        },
        ..Default::default()
    };

    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"event_id\":\"864ee97977bf43ac96d74f7486d138ab\",\
         \"extra_shit\":42,\"extra_garbage\":\"aha\"}"
    );
}

mod test_contexts {
    use super::*;

    #[test]
    fn test_device_context() {
        let event = v7::Event {
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "device".into(),
                    v7::DeviceContext {
                        name: Some("iphone".into()),
                        family: Some("iphone".into()),
                        model: Some("iphone7,3".into()),
                        model_id: Some("AH223".into()),
                        arch: Some("arm64".into()),
                        battery_level: Some(58.5.into()),
                        orientation: Some(v7::Orientation::Landscape),
                        simulator: Some(true),
                        memory_size: Some(3137978368),
                        free_memory: Some(322781184),
                        usable_memory: Some(2843525120),
                        storage_size: Some(63989469184),
                        free_storage: Some(31994734592),
                        external_storage_size: Some(2097152),
                        external_free_storage: Some(2097152),
                        boot_time: Some("2018-02-08T12:52:12Z".parse().unwrap()),
                        timezone: Some("Europe/Vienna".into()),
                    }.into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"contexts\":{\"device\":{\"name\":\"iphone\",\"family\":\"iphone\",\"model\":\
             \"iphone7,3\",\"model_id\":\"AH223\",\"arch\":\"arm64\",\"battery_level\":58.5,\
             \"orientation\":\"landscape\",\"simulator\":true,\"memory_size\":3137978368,\
             \"free_memory\":322781184,\"usable_memory\":2843525120,\"storage_size\":63989469184,\
             \"free_storage\":31994734592,\"external_storage_size\":2097152,\
             \"external_free_storage\":2097152,\"boot_time\":\"2018-02-08T12:52:12Z\",\"timezone\":\
             \"Europe/Vienna\",\"type\":\"device\"}}}"
        );
    }

    #[test]
    fn test_os_context() {
        let event = v7::Event {
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "os".into(),
                    v7::OsContext {
                        name: Some("iOS".into()),
                        version: Some("11.4.2".into()),
                        build: Some("ADSA23".into()),
                        kernel_version: Some("17.4.0".into()),
                        rooted: Some(true),
                    }.into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"contexts\":{\"os\":{\"name\":\"iOS\",\"version\":\"11.4.2\",\"build\":\"ADSA23\",\
             \"kernel_version\":\"17.4.0\",\"rooted\":true,\"type\":\"os\"}}}"
        );
    }

    #[test]
    fn test_app_context() {
        let event = v7::Event {
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "app".into(),
                    v7::AppContext {
                        app_start_time: Some("2018-02-08T22:21:57Z".parse().unwrap()),
                        device_app_hash: Some("4c793e3776474877ae30618378e9662a".into()),
                        build_type: Some("testflight".into()),
                        app_identifier: Some("foo.bar.baz".into()),
                        app_name: Some("Baz App".into()),
                        app_version: Some("1.0".into()),
                        app_build: Some("100001".into()),
                    }.into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"contexts\":{\"app\":{\"app_start_time\":\"2018-02-08T22:21:57Z\",\
             \"device_app_hash\":\"4c793e3776474877ae30618378e9662a\",\"build_type\":\
             \"testflight\",\"app_identifier\":\"foo.bar.baz\",\"app_name\":\"Baz App\",\
             \"app_version\":\"1.0\",\"app_build\":\"100001\",\"type\":\"app\"}}}"
        );
    }

    #[test]
    fn test_browser_context() {
        let event = v7::Event {
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "browser".into(),
                    v7::BrowserContext {
                        name: Some("Chrome".into()),
                        version: Some("59.0.3071".into()),
                    }.into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"contexts\":{\"browser\":{\"name\":\"Chrome\",\"version\":\"59.0.3071\",\"type\":\
             \"browser\"}}}"
        );
    }

    #[test]
    fn test_runtime_context() {
        let event = v7::Event {
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "runtime".into(),
                    v7::RuntimeContext {
                        name: Some("magicvm".into()),
                        version: Some("5.3".into()),
                    }.into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"contexts\":{\"runtime\":{\"name\":\"magicvm\",\"version\":\"5.3\",\"type\":\
             \"runtime\"}}}"
        );
    }

    #[test]
    fn test_unknown_context() {
        let event = v7::Event {
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "other".into(),
                    {
                        let mut m = v7::Map::new();
                        m.insert("aha".into(), "oho".into());
                        m
                    }.into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"contexts\":{\"other\":{\"type\":\"default\",\"aha\":\"oho\"}}}"
        );
    }

    #[test]
    fn test_renamed_contexts() {
        let event = v7::Event {
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "magicvm".into(),
                    v7::RuntimeContext {
                        name: Some("magicvm".into()),
                        version: Some("5.3".into()),
                    }.into(),
                );
                m.insert(
                    "othervm".into(),
                    v7::Context {
                        data: v7::RuntimeContext {
                            name: Some("magicvm".into()),
                            version: Some("5.3".into()),
                        }.into(),
                        other: {
                            let mut m = v7::Map::new();
                            m.insert("extra_stuff".into(), "extra_value".into());
                            m
                        },
                    },
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"contexts\":{\"magicvm\":{\"name\":\"magicvm\",\"version\":\"5.3\",\"type\":\
             \"runtime\"},\"othervm\":{\"name\":\"magicvm\",\"version\":\"5.3\",\"type\":\
             \"runtime\",\"extra_stuff\":\"extra_value\"}}}"
        );
    }

    #[test]
    fn test_contexts_interface() {
        assert_eq!(
            v7::Event {
                contexts: {
                    let mut m = v7::Map::new();
                    m.insert("os".into(), v7::OsContext::default().into());
                    m
                },
                ..Default::default()
            },
            serde_json::from_str("{\"sentry.interfaces.Contexts\":{\"os\":{\"type\":\"os\"}}}")
                .unwrap()
        );
    }
}

#[test]
fn test_level_log() {
    assert_eq!(v7::Level::Info, serde_json::from_str("\"log\"").unwrap());
}

#[test]
fn test_addr_format() {
    assert_eq!(serde_json::to_string(&v7::Addr(0)).unwrap(), "\"0x0\"");
    assert_eq!(serde_json::to_string(&v7::Addr(42)).unwrap(), "\"0x2a\"");
    assert_eq!(serde_json::from_str::<v7::Addr>("0").unwrap(), v7::Addr(0));
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0\"").unwrap(),
        v7::Addr(0)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0x0\"").unwrap(),
        v7::Addr(0)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("42").unwrap(),
        v7::Addr(42)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"42\"").unwrap(),
        v7::Addr(42)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0x2a\"").unwrap(),
        v7::Addr(42)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0X2A\"").unwrap(),
        v7::Addr(42)
    );
}

#[test]
fn test_addr_api() {
    use std::ptr;
    assert_eq!(v7::Addr::from(42u64), v7::Addr(42));
    assert_eq!(v7::Addr::from(42), v7::Addr(42));
    assert_eq!(v7::Addr::from(ptr::null::<()>()), v7::Addr(0));
}

#[test]
fn test_thread_id_format() {
    assert_eq!(serde_json::to_string(&v7::ThreadId::Int(0)).unwrap(), "0");
    assert_eq!(serde_json::to_string(&v7::ThreadId::Int(42)).unwrap(), "42");
    assert_eq!(
        serde_json::to_string(&v7::ThreadId::String("x".into())).unwrap(),
        "\"x\""
    );
    assert_eq!(
        serde_json::from_str::<v7::ThreadId>("0").unwrap(),
        v7::ThreadId::Int(0)
    );
    assert_eq!(
        serde_json::from_str::<v7::ThreadId>("\"0\"").unwrap(),
        v7::ThreadId::String("0".into())
    );
}

#[test]
fn test_orientation() {
    assert_eq!(
        serde_json::to_string(&v7::Orientation::Landscape).unwrap(),
        "\"landscape\""
    );
    assert_eq!(
        serde_json::to_string(&v7::Orientation::Portrait).unwrap(),
        "\"portrait\""
    );
}
