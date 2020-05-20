use sentry_core::protocol::{Breadcrumb, Event, Exception, Frame, Level, Map, Stacktrace, Value};
use slog::{OwnedKVList, Record, KV};

/// Converts a `slog::Level` to a sentry `Level`
pub fn convert_log_level(level: slog::Level) -> Level {
    match level {
        slog::Level::Trace | slog::Level::Debug => Level::Debug,
        slog::Level::Info => Level::Info,
        slog::Level::Warning => Level::Warning,
        slog::Level::Error | slog::Level::Critical => Level::Error,
    }
}

/// Adds the data from a `slog::KV` into a sentry `Map`.
fn add_kv_to_map(map: &mut Map<String, Value>, kv: &impl KV) {
    let _ = (map, kv);
    // TODO: actually implement this ;-)
}

/// Creates a sentry `Breadcrumb` from the `slog::Record`.
pub fn breadcrumb_from_record(record: &Record, values: &OwnedKVList) -> Breadcrumb {
    let mut data = Map::new();
    add_kv_to_map(&mut data, &record.kv());
    add_kv_to_map(&mut data, values);

    Breadcrumb {
        ty: "log".into(),
        message: Some(record.msg().to_string()),
        level: convert_log_level(record.level()),
        data,
        ..Default::default()
    }
}

/// Creates a simple message `Event` from the `slog::Record`.
pub fn event_from_record(record: &Record, values: &OwnedKVList) -> Event<'static> {
    let mut extra = Map::new();
    add_kv_to_map(&mut extra, &record.kv());
    add_kv_to_map(&mut extra, values);
    Event {
        message: Some(record.msg().to_string()),
        level: convert_log_level(record.level()),
        ..Default::default()
    }
}

/// Creates an exception `Event` from the `slog::Record`.
///
/// The exception will have a stacktrace that corresponds to the location
/// information contained in the `slog::Record`.
///
/// # Examples
///
/// ```
/// let args = format_args!("");
/// let record = slog::record!(slog::Level::Error, "", &args, slog::b!());
/// let kv = slog::o!().into();
/// let event = sentry_slog::exception_from_record(&record, &kv);
///
/// let frame = &event.exception.as_ref()[0]
///     .stacktrace
///     .as_ref()
///     .unwrap()
///     .frames[0];
/// assert!(frame.lineno.unwrap() > 0);
/// ```
pub fn exception_from_record(record: &Record, values: &OwnedKVList) -> Event<'static> {
    let mut event = event_from_record(record, values);
    let frame = Frame {
        function: Some(record.function().into()),
        module: Some(record.module().into()),
        filename: Some(record.file().into()),
        lineno: Some(record.line().into()),
        colno: Some(record.column().into()),
        ..Default::default()
    };
    let exception = Exception {
        ty: "slog::Record".into(),
        stacktrace: Some(Stacktrace {
            frames: vec![frame],
            ..Default::default()
        }),
        ..Default::default()
    };
    event.exception = vec![exception].into();
    event
}
