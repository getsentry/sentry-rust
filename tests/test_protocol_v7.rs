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
fn test_basic_event() {
    let mut event: v7::Event = Default::default();
    event.logentry = Some(v7::LogEntry {
        message: "Hello %s!".into(),
        params: vec!["Peter!".into()],
    });
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
        "{\"logentry\":{\"message\":\"Hello %s!\",\"params\":[\"Peter!\"]},\
         \"contexts\":{\"os\":{\"name\":\"linux\",\"rooted\":true,\"type\":\
         \"os\"}}}"
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
