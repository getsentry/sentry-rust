extern crate sentry_types;
extern crate serde;
extern crate serde_json;
extern crate uuid;

use std::collections::HashMap;

use sentry_types::protocol::v7;

fn reserialize(event: &v7::Event) -> v7::Event {
    let json = serde_json::to_string(event).unwrap();
    serde_json::from_str(&json).unwrap()
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
fn test_basic_event() {
    let mut event = v7::Event {
        id: "d43e86c9-6e42-4a93-a4fb-da156dd17341".parse().ok(),
        logentry: Some(v7::LogEntry {
            message: "Hello %s!".into(),
            params: vec!["Peter!".into()],
        }),
        ..Default::default()
    };
    event.contexts.insert(
        "os".into(),
        v7::ContextType::Os(v7::OsContext {
            name: Some("linux".into()),
            rooted: Some(true),
            ..Default::default()
        }).into(),
    );

    let json = serde_json::to_string(&event).unwrap();
    let event2: v7::Event = serde_json::from_str(&json).unwrap();

    assert_eq!(&event, &event2);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"logentry\":\
         {\"message\":\"Hello %s!\",\"params\":[\"Peter!\"]},\
         \"contexts\":{\"os\":{\"name\":\"linux\",\"rooted\":true,\"type\":\
         \"os\"}}}"
    );
}

#[test]
fn test_release_and_dist() {
    let event = v7::Event {
        dist: Some("42".to_string()),
        release: Some("my.awesome.app-1.0".to_string()),
        environment: Some("prod".to_string()),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"release\":\"my.awesome.app-1.0\",\"dist\":\"42\",\"environment\":\"prod\"}"
    );
}

#[test]
fn test_fingerprint() {
    let mut event: v7::Event = Default::default();
    assert_eq!(serde_json::to_string(&event).unwrap(), "{}");

    event.fingerprint.push("extra".into());
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"fingerprint\":[\"{{ default }}\",\"extra\"]}"
    );
}

#[test]
fn test_basic_message_event() {
    let mut event: v7::Event = Default::default();
    event.level = v7::Level::Warning;
    event.message = Some("Hello World!".into());
    event.logger = Some("root".into());
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(
        &json,
        "{\"level\":\"warning\",\"message\":\"Hello World!\",\"logger\":\"root\"}"
    );
}

#[test]
fn test_message_basics() {
    let event = v7::Event {
        message: Some("Hello World!".to_string()),
        culprit: Some("foo in bar".to_string()),
        level: v7::Level::Info,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"level\":\"info\",\"culprit\":\"foo in bar\",\"message\":\"Hello World!\"}"
    );
}

#[test]
fn test_logentry_basics() {
    let event = v7::Event {
        logentry: Some(v7::LogEntry {
            message: "Hello %s!".to_string(),
            params: vec!["World".into()],
        }),
        culprit: Some("foo in bar".to_string()),
        level: v7::Level::Debug,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"level\":\"debug\",\"culprit\":\"foo in bar\",\"logentry\":{\"message\":\
         \"Hello %s!\",\"params\":[\"World\"]}}"
    );
}

#[test]
fn test_modules() {
    let event = v7::Event {
        modules: {
            let mut m = HashMap::new();
            m.insert("System".into(), "1.0.0".into());
            m
        },
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"modules\":{\"System\":\"1.0.0\"}}"
    );
}

#[test]
fn test_repos() {
    let event = v7::Event {
        repos: {
            let mut m = HashMap::new();
            m.insert("/raven".into(), v7::RepoReference {
                name: "github/raven".into(),
                prefix: Some("/".into()),
                revision: Some("49f45700b5fe606c1bcd9bf0205ecbb83db17f52".into()),
            });
            m
        },
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"repos\":{\"/raven\":{\"name\":\"github/raven\",\"prefix\":\"/\",\"revision\":\"49f45700b5fe606c1bcd9bf0205ecbb83db17f52\"}}}"
    );

    let event = v7::Event {
        repos: {
            let mut m = HashMap::new();
            m.insert("/raven".into(), v7::RepoReference {
                name: "github/raven".into(),
                prefix: None,
                revision: None,
            });
            m
        },
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"repos\":{\"/raven\":{\"name\":\"github/raven\"}}}"
    );
}

#[test]
fn test_user() {
    let event = v7::Event {
        user: Some(v7::User {
            id: Some("8fd5a33b-5b0e-45b2-aff2-9e4f067756ba".into()),
            email: Some("foo@example.invalid".into()),
            ip_address: Some("127.0.0.1".parse().unwrap()),
            username: Some("john-doe".into()),
            data: {
                let mut hm = HashMap::new();
                hm.insert("foo".into(), "bar".into());
                hm
            }
        }),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"user\":{\"id\":\"8fd5a33b-5b0e-45b2-aff2-9e4f067756ba\",\
         \"email\":\"foo@example.invalid\",\"ip_address\":\"127.0.0.1\",\
         \"username\":\"john-doe\",\"foo\":\"bar\"}}"
    );

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

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"user\":{\"id\":\"8fd5a33b-5b0e-45b2-aff2-9e4f067756ba\"}}"
    );
}

#[test]
fn test_request() {
    let event = v7::Event {
        request: Some(v7::Request {
            url: "https://www.example.invalid/bar".parse().ok(),
            method: Some("GET".into()),
            data: Some("{}".into()),
            query_string: Some("foo=bar&blub=blah".into()),
            cookies: Some("dummy=42".into()),
            headers: {
                let mut hm = HashMap::new();
                hm.insert("Content-Type".into(), "text/plain".into());
                hm
            },
            env: {
                let mut env = HashMap::new();
                env.insert("PATH_INFO".into(), "/bar".into());
                env
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"request\":{\"url\":\"https://www.example.invalid/bar\",\
         \"method\":\"GET\",\"data\":\"{}\",\"query_string\":\
         \"foo=bar&blub=blah\",\"cookies\":\"dummy=42\",\"headers\":\
         {\"Content-Type\":\"text/plain\"},\"env\":\
         {\"PATH_INFO\":\"/bar\"}}}"
    );

    let event = v7::Event {
        request: Some(v7::Request {
            url: "https://www.example.invalid/bar".parse().ok(),
            method: Some("GET".into()),
            data: Some("{}".into()),
            query_string: Some("foo=bar&blub=blah".into()),
            cookies: Some("dummy=42".into()),
            other: {
                let mut m = HashMap::new();
                m.insert("other_key".into(), "other_value".into());
                m
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"request\":{\"url\":\"https://www.example.invalid/bar\",\
         \"method\":\"GET\",\"data\":\"{}\",\"query_string\":\
         \"foo=bar&blub=blah\",\"cookies\":\"dummy=42\",\
         \"other_key\":\"other_value\"}}"
    );

    let event = v7::Event {
        request: Some(Default::default()),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"request\":{}}"
    );
}

#[test]
fn test_canonical_exception() {
    let mut event: v7::Event = Default::default();
    event.exceptions.push(v7::Exception {
        ty: "ZeroDivisionError".into(),
        ..Default::default()
    });
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(
        json,
        "{\"exception\":{\"values\":[{\"type\":\"ZeroDivisionError\"}]}}"
    );

    let event2: v7::Event = serde_json::from_str(&json).unwrap();
    assert_eq!(event, event2);
}

#[test]
fn test_single_exception_inline() {
    let json = "{\"exception\":{\"type\":\"ZeroDivisionError\"}}";
    let event: v7::Event = serde_json::from_str(&json).unwrap();
    let mut ref_event: v7::Event = Default::default();
    ref_event.exceptions.push(v7::Exception {
        ty: "ZeroDivisionError".into(),
        ..Default::default()
    });
    assert_eq!(event, ref_event);
}

#[test]
fn test_multi_exception_list() {
    let json = "{\"exception\":[{\"type\":\"ZeroDivisionError\"}]}";
    let event: v7::Event = serde_json::from_str(&json).unwrap();
    let mut ref_event: v7::Event = Default::default();
    ref_event.exceptions.push(v7::Exception {
        ty: "ZeroDivisionError".into(),
        ..Default::default()
    });
    assert_eq!(event, ref_event);
}

#[test]
fn test_minimal_exception_stacktrace() {
    let event: v7::Event = v7::Event {
        exceptions: vec![v7::Exception {
            ty: "DivisionByZero".into(),
            value: Some("integer division or modulo by zero".into()),
            module: None,
            stacktrace: Some(v7::Stacktrace {
                frames: vec![
                    v7::Frame {
                        function: Some("main".into()),
                        location: v7::FileLocation {
                            filename: Some("hello.py".into()),
                            line: Some(1),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                ],
                ..Default::default()
            }),
        }],
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\
         \"value\":\"integer division or modulo by zero\",\"stacktrace\":\
         {\"frames\":[{\"function\":\"main\",\"filename\":\"hello.py\",\
         \"lineno\":1}]}}]}}"
    );
}

#[test]
fn test_slightly_larger_exception_stacktrace() {
    let event: v7::Event = v7::Event {
        exceptions: vec![v7::Exception {
            ty: "DivisionByZero".into(),
            value: Some("integer division or modulo by zero".into()),
            module: None,
            stacktrace: Some(v7::Stacktrace {
                frames: vec![
                    v7::Frame {
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
                            let mut m = HashMap::new();
                            m.insert("var".into(), "value".into());
                            m
                        },
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
        }],
        ..Default::default()
    };

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
fn test_full_exception_stacktrace() {
    let event: v7::Event = v7::Event {
        exceptions: vec![v7::Exception {
            ty: "DivisionByZero".into(),
            value: Some("integer division or modulo by zero".into()),
            module: Some("x".into()),
            stacktrace: Some(v7::Stacktrace {
                frames: vec![
                    v7::Frame {
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
                            let mut m = HashMap::new();
                            m.insert("var".into(), "value".into());
                            m
                        },
                        package: Some("hello.whl".into()),
                        module: Some("hello".into()),
                        instruction_info: v7::InstructionInfo {
                            image_addr: Some(0),
                            instruction_addr: Some(0),
                            symbol_addr: Some(0),
                        }
                    },
                ],
                frames_omitted: Some((1, 2)),
            }),
        }],
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\
         \"value\":\"integer division or modulo by zero\",\"module\":\
         \"x\",\"stacktrace\":{\"frames\":[{\"function\":\"main\",\"symbol\":\
         \"main\",\"module\":\"hello\",\"package\":\"hello.whl\",\"filename\":\
         \"hello.py\",\"abs_path\":\"/app/hello.py\",\"lineno\":7,\"\
         colno\":42,\"pre_context\":[\"foo\",\"bar\"],\"context_line\":\
         \"hey hey hey\",\"post_context\":[\"foo\",\"bar\"],\"in_app\":true,\
         \"vars\":{\"var\":\"value\"},\"image_addr\":0,\"instruction_addr\":0,\
         \"symbol_addr\":0}],\"frames_omitted\":[1,2]}}]}}"
    );
}

#[test]
fn test_addr_format() {
    assert_eq!(serde_json::to_string(&v7::Addr(0)).unwrap(), "\"0x0\"");
    assert_eq!(serde_json::to_string(&v7::Addr(42)).unwrap(), "\"0x2a\"");
    assert_eq!(serde_json::from_str::<v7::Addr>("0").unwrap(), v7::Addr(0));
    assert_eq!(serde_json::from_str::<v7::Addr>("\"0\"").unwrap(), v7::Addr(0));
    assert_eq!(serde_json::from_str::<v7::Addr>("\"0x0\"").unwrap(), v7::Addr(0));
    assert_eq!(serde_json::from_str::<v7::Addr>("42").unwrap(), v7::Addr(42));
    assert_eq!(serde_json::from_str::<v7::Addr>("\"42\"").unwrap(), v7::Addr(42));
    assert_eq!(serde_json::from_str::<v7::Addr>("\"0x2a\"").unwrap(), v7::Addr(42));
    assert_eq!(serde_json::from_str::<v7::Addr>("\"0X2A\"").unwrap(), v7::Addr(42));
}

#[test]
fn test_thread_id_format() {
    assert_eq!(serde_json::to_string(&v7::ThreadId::Int(0)).unwrap(), "0");
    assert_eq!(serde_json::to_string(&v7::ThreadId::Int(42)).unwrap(), "42");
    assert_eq!(serde_json::to_string(&v7::ThreadId::String("x".into())).unwrap(), "\"x\"");
    assert_eq!(serde_json::from_str::<v7::ThreadId>("0").unwrap(), v7::ThreadId::Int(0));
    assert_eq!(serde_json::from_str::<v7::ThreadId>("\"0\"").unwrap(), v7::ThreadId::String("0".into()));
}
