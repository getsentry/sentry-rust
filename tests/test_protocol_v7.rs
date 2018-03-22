extern crate sentry_types;
extern crate serde;
extern crate serde_json;

use sentry_types::protocol::v7;

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
