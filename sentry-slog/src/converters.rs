use sentry_core::protocol::{Breadcrumb, Event, Level, Map, Value};
#[cfg(feature = "logs")]
use sentry_core::protocol::{Log, LogAttribute, LogLevel};
use slog::{Key, OwnedKVList, Record, Serializer, KV};
#[cfg(feature = "logs")]
use std::collections::BTreeMap;
use std::fmt;
#[cfg(feature = "logs")]
use std::time::SystemTime;

/// Converts a [`slog::Level`] to a Sentry [`Level`]
pub fn convert_log_level(level: slog::Level) -> Level {
    match level {
        slog::Level::Trace | slog::Level::Debug => Level::Debug,
        slog::Level::Info => Level::Info,
        slog::Level::Warning => Level::Warning,
        slog::Level::Error | slog::Level::Critical => Level::Error,
    }
}

/// Converts a [`slog::Level`] to a Sentry [`LogLevel`], used for [`Log`].
#[cfg(feature = "logs")]
pub fn convert_to_sentry_log_level(level: slog::Level) -> LogLevel {
    match level {
        slog::Level::Trace => LogLevel::Trace,
        slog::Level::Debug => LogLevel::Debug,
        slog::Level::Info => LogLevel::Info,
        slog::Level::Warning => LogLevel::Warn,
        slog::Level::Error => LogLevel::Error,
        slog::Level::Critical => LogLevel::Fatal,
    }
}

struct MapSerializer<'a>(&'a mut Map<String, Value>);

macro_rules! impl_into {
    ($t:ty => $f:ident) => {
        fn $f(&mut self, key: Key, val: $t) -> slog::Result {
            self.0.insert(key.into(), val.into());
            Ok(())
        }
    };
}
impl Serializer for MapSerializer<'_> {
    fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> slog::Result {
        self.0.insert(key.into(), val.to_string().into());
        Ok(())
    }

    fn emit_serde(&mut self, key: Key, val: &dyn slog::SerdeValue) -> slog::Result {
        let value = serde_json::to_value(val.as_serde()).map_err(|_e| slog::Error::Other)?;
        self.0.insert(key.into(), value);
        Ok(())
    }

    impl_into! { usize => emit_usize }
    impl_into! { isize => emit_isize }
    impl_into! { bool  => emit_bool  }
    impl_into! { u8    => emit_u8    }
    impl_into! { i8    => emit_i8    }
    impl_into! { u16   => emit_u16   }
    impl_into! { i16   => emit_i16   }
    impl_into! { u32   => emit_u32   }
    impl_into! { i32   => emit_i32   }
    impl_into! { f32   => emit_f32   }
    impl_into! { u64   => emit_u64   }
    impl_into! { i64   => emit_i64   }
    impl_into! { f64   => emit_f64   }
    impl_into! { &str  => emit_str   }
}

/// Adds the data from a [`slog::KV`] into a Sentry [`Map`].
fn add_kv_to_map(map: &mut Map<String, Value>, record: &Record, kv: &impl KV) {
    // TODO: Do something with these errors?
    let _ = record.kv().serialize(record, &mut MapSerializer(map));
    let _ = kv.serialize(record, &mut MapSerializer(map));
}

/// Creates a Sentry [`Breadcrumb`] from the [`Record`].
pub fn breadcrumb_from_record(record: &Record, values: &OwnedKVList) -> Breadcrumb {
    let mut data = Map::new();
    add_kv_to_map(&mut data, record, values);

    Breadcrumb {
        ty: "log".into(),
        message: Some(record.msg().to_string()),
        level: convert_log_level(record.level()),
        data,
        ..Default::default()
    }
}

/// Creates a simple message [`Event`] from the [`Record`].
pub fn event_from_record(record: &Record, values: &OwnedKVList) -> Event<'static> {
    let mut extra = Map::new();
    add_kv_to_map(&mut extra, record, values);
    Event {
        message: Some(record.msg().to_string()),
        level: convert_log_level(record.level()),
        extra,
        ..Default::default()
    }
}

/// Creates an exception [`Event`] from the [`Record`].
///
/// # Examples
///
/// ```
/// let args = format_args!("");
/// let record = slog::record!(slog::Level::Error, "", &args, slog::b!());
/// let kv = slog::o!().into();
/// let event = sentry_slog::exception_from_record(&record, &kv);
/// ```
pub fn exception_from_record(record: &Record, values: &OwnedKVList) -> Event<'static> {
    // TODO: Exception records in Sentry need a valid type, value and full stack trace to support
    // proper grouping and issue metadata generation. log::Record does not contain sufficient
    // information for this. However, it may contain a serialized error which we can parse to emit
    // an exception record.
    event_from_record(record, values)
}

/// Creates a [`Log`] from the [`Record`].
///
/// The record's key-value pairs are attached as log attributes, alongside the
/// source code location the record originated from.
#[cfg(feature = "logs")]
pub fn log_from_record(record: &Record, values: &OwnedKVList) -> Log {
    let mut kvs = Map::new();
    add_kv_to_map(&mut kvs, record, values);

    let mut attributes: BTreeMap<String, LogAttribute> = kvs
        .into_iter()
        .map(|(key, val)| (key, val.into()))
        .collect();

    attributes.insert("code.file.path".into(), record.file().into());
    attributes.insert("code.line.number".into(), record.line().into());
    attributes.insert("code.module.name".into(), record.module().into());

    let tag = record.tag();
    if !tag.is_empty() {
        attributes.insert("logger.name".into(), tag.into());
    }

    attributes.insert("sentry.origin".into(), "auto.logger.slog".into());

    Log {
        level: convert_to_sentry_log_level(record.level()),
        body: record.msg().to_string(),
        trace_id: None,
        timestamp: SystemTime::now(),
        severity_number: None,
        attributes,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::Serialize;

    use slog::{b, o, record, Level};

    #[derive(Serialize, Clone)]
    struct Something {
        msg: String,
        count: usize,
    }

    impl slog::Value for Something {
        fn serialize(
            &self,
            _record: &Record,
            key: Key,
            serializer: &mut dyn slog::Serializer,
        ) -> slog::Result {
            serializer.emit_serde(key, self)
        }
    }

    impl slog::SerdeValue for Something {
        fn as_serde(&self) -> &dyn erased_serde::Serialize {
            self
        }

        fn to_sendable(&self) -> Box<dyn slog::SerdeValue + Send + 'static> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn test_slog_kvs() {
        let extras = o!("lib" => "sentry", "version" => 1, "test" => true);

        let mut map: Map<String, Value> = Map::new();

        add_kv_to_map(
            &mut map,
            &record!(
                Level::Debug,
                "test",
                &format_args!("Hello, world!"),
                b!("something" => &Something {
                    msg: "message!".into(),
                    count: 42,
                })
            ),
            &extras,
        );

        assert_eq!(map.get("lib"), Some(&"sentry".into()));
        assert_eq!(map.get("version"), Some(&1.into()));
        assert_eq!(map.get("test"), Some(&true.into()));
        assert_eq!(
            map.get("something"),
            Some(&Value::Object(
                vec![
                    ("msg".to_string(), Value::from("message!")),
                    ("count".to_string(), Value::from(42))
                ]
                .into_iter()
                .collect()
            ))
        )
    }

    #[cfg(feature = "logs")]
    #[test]
    fn test_log_from_record() {
        let kv = o!("user_id" => 42, "request_id" => "abc123");
        let args = format_args!("Hello, world!");
        let record = record!(Level::Info, "some_tag", &args, b!());

        let log = log_from_record(&record, &kv.into());

        assert_eq!(log.level, LogLevel::Info);
        assert_eq!(log.body, "Hello, world!");
        assert_eq!(log.attributes.get("user_id").unwrap().0, Value::from(42));
        assert_eq!(
            log.attributes.get("request_id").unwrap().0,
            Value::from("abc123")
        );
        assert_eq!(
            log.attributes.get("logger.name").unwrap().0,
            Value::from("some_tag")
        );
        assert_eq!(
            log.attributes.get("sentry.origin").unwrap().0,
            Value::from("auto.logger.slog")
        );
        assert!(log.attributes.contains_key("code.module.name"));
    }
}
