extern crate sentry_types;
extern crate serde;
extern crate serde_json;
extern crate uuid;

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
